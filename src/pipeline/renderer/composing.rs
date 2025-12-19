use crate::MapRenderError;
use crate::pipeline::api::{Anchor, Document, PreparedDataSources};
use crate::pipeline::context::PipelineContext;
use crate::pipeline::renderer::RenderingStrategy;
use log::{info, warn};
use lopdf::{Document as LopdfDocument, Object, ObjectId, StringFormat, dictionary};
use petty_core::error::PipelineError;
use petty_core::layout::{LayoutEngine, LayoutStore};
use petty_core::parser::processor::{DataSourceFormat, ExecutionConfig};
use petty_layout::{LayoutElement, PositionedElement};
use petty_pdf_composer::{merge_documents, overlay_content};
use petty_render_core::DocumentRenderer;
use petty_render_lopdf::LopdfRenderer;
use serde_json::json;
use std::collections::HashMap;
use std::io::{Cursor, Seek, Write};

#[derive(Clone)]
pub struct ComposingRenderer;

struct PendingLink {
    local_page_idx: usize,
    rect: [f32; 4],
    target_id: String,
}

impl RenderingStrategy for ComposingRenderer {
    fn render<W>(
        &self,
        context: &PipelineContext,
        sources: PreparedDataSources,
        mut writer: W,
    ) -> Result<W, PipelineError>
    where
        W: Write + Seek + Send + 'static,
    {
        let doc_metadata = sources.document.ok_or_else(|| {
            PipelineError::Config(
                "ComposingRenderer requires a Document metadata object, but none was provided."
                    .to_string(),
            )
        })?;

        let mut body_artifact = sources.body_artifact.ok_or_else(|| {
            PipelineError::Config(
                "ComposingRenderer requires a pre-rendered body artifact, but none was provided."
                    .to_string(),
            )
        })?;

        info!("[COMPOSER] Loading temporary body artifact for composition.");
        let mut main_doc = LopdfDocument::load_from(&mut body_artifact)?;
        info!(
            "[COMPOSER] Body artifact loaded with {} pages.",
            main_doc.get_pages().len()
        );

        let original_body_page_ids: Vec<ObjectId> =
            main_doc.get_pages().values().cloned().collect();
        let anchor_map: HashMap<String, &Anchor> = doc_metadata
            .anchors
            .iter()
            .map(|a| (a.id.clone(), a))
            .collect();

        let stylesheet = context.compiled_template.stylesheet();
        let (page_width, page_height) = stylesheet.get_default_page_layout().size.dimensions_pt();
        let mut prepended_pages = 0;

        let prepend_roles = ["cover-page", "preface", "table-of-contents"];
        let append_roles = ["back-cover"];

        for role in prepend_roles.iter().rev() {
            if let Some(template) = context.role_templates.get(*role) {
                info!("[COMPOSER] Executing prepend role template: '{}'", role);
                let doc_json_str = serde_json::to_string(&*doc_metadata)?;
                let exec_config = ExecutionConfig {
                    format: DataSourceFormat::Json,
                    strict: false,
                };
                let ir_nodes = template.execute(&doc_json_str, exec_config)?;

                let layout_engine = LayoutEngine::new(&context.font_library, context.cache_config);
                // Pass Arc<Stylesheet> correctly
                let mut temp_renderer =
                    LopdfRenderer::new(layout_engine, stylesheet.clone()).map_render_err()?;
                temp_renderer
                    .begin_document(Cursor::new(Vec::new()))
                    .map_render_err()?;

                let store = LayoutStore::new();
                let ir_root = petty_core::idf::IRNode::Root(ir_nodes);
                let root_node = temp_renderer
                    .layout_engine
                    .build_render_tree(&ir_root, &store)
                    .map_err(|e| PipelineError::Layout(e.to_string()))?;

                let iterator = temp_renderer
                    .layout_engine
                    .paginate(&stylesheet, root_node, &store)
                    .map_err(|e| PipelineError::Layout(e.to_string()))?;

                let mut laid_out_pages = Vec::new();
                for page_res in iterator {
                    laid_out_pages.push(page_res?.elements);
                }

                if !laid_out_pages.is_empty() {
                    let pending_links = collect_links_from_layout(&laid_out_pages);

                    let mut new_page_ids = vec![];
                    // Use registered_fonts() to get all fonts from both fontdb and FontProvider
                    let font_map: HashMap<String, String> = temp_renderer
                        .layout_engine
                        .registered_fonts()
                        .iter()
                        .enumerate()
                        .map(|(i, f)| (f.postscript_name.clone(), format!("F{}", i + 1)))
                        .collect();

                    for page_elements in laid_out_pages {
                        let content_id = temp_renderer
                            .render_page_content(page_elements, &font_map, page_width, page_height)
                            .map_render_err()?;
                        let page_id = temp_renderer
                            .write_page_object(vec![content_id], vec![], page_width, page_height)
                            .map_render_err()?;
                        new_page_ids.push(page_id);
                    }

                    // Call finish_into_buffer directly on the concrete type, not Box
                    let role_pdf_bytes = temp_renderer
                        .finish_into_buffer(new_page_ids)
                        .map_render_err()?;
                    let role_doc = LopdfDocument::load_mem(&role_pdf_bytes)?;
                    let role_page_count = role_doc.get_pages().len();

                    info!(
                        "[COMPOSER] Prepending {} pages from role '{}'",
                        role_page_count, role
                    );
                    prepended_pages += role_page_count;
                    merge_documents(&mut main_doc, role_doc, true)
                        .map_err(|e| PipelineError::Render(e.to_string()))?;

                    let current_pages = main_doc.get_pages();

                    for link in pending_links {
                        if let Some(anchor) = anchor_map.get(&link.target_id) {
                            let body_page_idx = anchor.page_number.saturating_sub(1);
                            if let Some(target_obj_id) = original_body_page_ids.get(body_page_idx) {
                                let source_page_num = (link.local_page_idx + 1) as u32;
                                if let Some(source_page_id) = current_pages.get(&source_page_num) {
                                    create_link_annotation(
                                        &mut main_doc,
                                        *source_page_id,
                                        *target_obj_id,
                                        link.rect,
                                        page_height,
                                        anchor.y_position,
                                    )?;
                                }
                            }
                        }
                    }
                }
            }
        }

        for role in append_roles.iter() {
            if let Some(template) = context.role_templates.get(*role) {
                info!("[COMPOSER] Executing append role template: '{}'", role);
                let doc_json_str = serde_json::to_string(&*doc_metadata)?;
                let exec_config = ExecutionConfig {
                    format: DataSourceFormat::Json,
                    strict: false,
                };
                let ir_nodes = template.execute(&doc_json_str, exec_config)?;

                let layout_engine = LayoutEngine::new(&context.font_library, context.cache_config);
                let mut temp_renderer =
                    LopdfRenderer::new(layout_engine, stylesheet.clone()).map_render_err()?;
                temp_renderer
                    .begin_document(Cursor::new(Vec::new()))
                    .map_render_err()?;

                let store = LayoutStore::new();
                let ir_root = petty_core::idf::IRNode::Root(ir_nodes);
                let root_node = temp_renderer
                    .layout_engine
                    .build_render_tree(&ir_root, &store)
                    .map_err(|e| PipelineError::Layout(e.to_string()))?;

                let iterator = temp_renderer
                    .layout_engine
                    .paginate(&stylesheet, root_node, &store)
                    .map_err(|e| PipelineError::Layout(e.to_string()))?;

                let mut laid_out_pages = Vec::new();
                for page_res in iterator {
                    laid_out_pages.push(page_res?.elements);
                }

                if !laid_out_pages.is_empty() {
                    let mut new_page_ids = vec![];
                    // Use registered_fonts() to get all fonts from both fontdb and FontProvider
                    let font_map: HashMap<String, String> = temp_renderer
                        .layout_engine
                        .registered_fonts()
                        .iter()
                        .enumerate()
                        .map(|(i, f)| (f.postscript_name.clone(), format!("F{}", i + 1)))
                        .collect();

                    for page_elements in laid_out_pages {
                        let content_id = temp_renderer
                            .render_page_content(page_elements, &font_map, page_width, page_height)
                            .map_render_err()?;
                        let page_id = temp_renderer
                            .write_page_object(vec![content_id], vec![], page_width, page_height)
                            .map_render_err()?;
                        new_page_ids.push(page_id);
                    }

                    let role_pdf_bytes = temp_renderer
                        .finish_into_buffer(new_page_ids)
                        .map_render_err()?;
                    let role_doc = LopdfDocument::load_mem(&role_pdf_bytes)?;

                    info!(
                        "[COMPOSER] Appending {} pages from role '{}'",
                        role_doc.get_pages().len(),
                        role
                    );
                    merge_documents(&mut main_doc, role_doc, false)
                        .map_err(|e| PipelineError::Render(e.to_string()))?;
                }
            }
        }

        let final_page_count = main_doc.get_pages().len();
        let overlay_roles = ["page-header", "page-footer"];
        let page_ids: Vec<ObjectId> = main_doc.get_pages().into_values().collect();

        for (role, template) in context.role_templates.iter() {
            if overlay_roles.contains(&role.as_str()) {
                info!("[COMPOSER] Executing overlay role template: '{}'", role);
                let layout_engine = LayoutEngine::new(&context.font_library, context.cache_config);

                for (i, page_id) in page_ids.iter().enumerate() {
                    let page_number = i + 1;
                    let overlay_context_val = json!({
                        "document": &*doc_metadata, "page_number": page_number, "page_count": final_page_count
                    });
                    let overlay_context_str = serde_json::to_string(&overlay_context_val)?;
                    let exec_config = ExecutionConfig {
                        format: DataSourceFormat::Json,
                        strict: false,
                    };
                    let ir_nodes = template.execute(&overlay_context_str, exec_config)?;

                    let store = LayoutStore::new();
                    let ir_root = petty_core::idf::IRNode::Root(ir_nodes);
                    let root_node = layout_engine
                        .build_render_tree(&ir_root, &store)
                        .map_err(|e| PipelineError::Layout(e.to_string()))?;

                    let iterator = layout_engine
                        .paginate(&stylesheet, root_node, &store)
                        .map_err(|e| PipelineError::Layout(e.to_string()))?;

                    let mut overlay_pages = Vec::new();
                    for page_res in iterator {
                        overlay_pages.push(page_res?.elements);
                    }

                    if let Some(elements) = overlay_pages.pop() {
                        if !overlay_pages.is_empty() {
                            warn!(
                                "[COMPOSER] Overlay template for role '{}' generated more than one page of content. Only the first will be used.",
                                role
                            );
                        }
                        // Use registered_fonts() to get all fonts from both fontdb and FontProvider
                        let font_map: HashMap<String, String> = layout_engine
                            .registered_fonts()
                            .iter()
                            .enumerate()
                            .map(|(i, f)| (f.postscript_name.clone(), format!("F{}", i + 1)))
                            .collect();
                        let content = petty_render_lopdf::render_elements_to_content(
                            elements,
                            &font_map,
                            page_width,
                            page_height,
                        )
                        .map_render_err()?;
                        overlay_content(&mut main_doc, *page_id, content.encode()?)
                            .map_err(|e| PipelineError::Render(e.to_string()))?;
                    }
                }
            }
        }

        if !doc_metadata.hyperlinks.is_empty() || !doc_metadata.headings.is_empty() {
            info!("[COMPOSER] Applying fixups (body links, outlines).");
            let final_page_ids: Vec<ObjectId> = main_doc.get_pages().into_values().collect();
            let root_id = main_doc.trailer.get(b"Root")?.as_reference()?;

            let annots_by_page = create_link_annotations_for_body(
                &mut main_doc,
                &doc_metadata,
                &final_page_ids,
                page_height,
                prepended_pages,
            )?;
            for (page_idx, annot_ids) in annots_by_page {
                if page_idx > 0 && page_idx <= final_page_ids.len() {
                    let page_id = final_page_ids[page_idx - 1];
                    append_annotations_to_page(&mut main_doc, page_id, annot_ids)?;
                }
            }

            if let Some(outline_root_id) = build_outlines_for_doc(
                &mut main_doc,
                &doc_metadata,
                &final_page_ids,
                page_height,
                prepended_pages,
            )? && let Ok(Object::Dictionary(root_dict_mut)) = main_doc.get_object_mut(root_id)
            {
                root_dict_mut.set("Outlines", outline_root_id);
                root_dict_mut.set("PageMode", "UseOutlines");
            }
        } else {
            info!("[COMPOSER] No fixups required. Passing document through.");
        }

        main_doc.save_to(&mut writer)?;
        info!("[COMPOSER] Composition complete. Final document saved.");

        Ok(writer)
    }
}

fn collect_links_from_layout(pages: &[Vec<PositionedElement>]) -> Vec<PendingLink> {
    let mut links = Vec::new();
    for (page_idx, elements) in pages.iter().enumerate() {
        for el in elements {
            if let LayoutElement::Text(text_node) = &el.element
                && let Some(href) = &text_node.href
                && let Some(target) = href.strip_prefix('#')
            {
                links.push(PendingLink {
                    local_page_idx: page_idx,
                    rect: [el.x, el.y, el.x + el.width, el.y + el.height],
                    target_id: target.to_string(),
                });
            }
        }
    }
    links
}

fn append_annotations_to_page(
    doc: &mut LopdfDocument,
    page_id: ObjectId,
    annot_ids: Vec<ObjectId>,
) -> Result<(), PipelineError> {
    let annots_array_ref = if let Ok(Object::Dictionary(page_dict)) = doc.get_object(page_id) {
        match page_dict.get(b"Annots") {
            Ok(Object::Array(_)) => None,
            Ok(Object::Reference(ref_id)) => Some(*ref_id),
            _ => None,
        }
    } else {
        return Ok(());
    };

    if let Some(ref_id) = annots_array_ref {
        if let Ok(Object::Array(annots)) = doc.get_object_mut(ref_id) {
            for id in annot_ids {
                annots.push(Object::Reference(id));
            }
        }
    } else if let Ok(Object::Dictionary(page_dict)) = doc.get_object_mut(page_id) {
        if page_dict.has(b"Annots") {
            if let Ok(Object::Array(annots)) = page_dict.get_mut(b"Annots") {
                for id in annot_ids {
                    annots.push(Object::Reference(id));
                }
            }
        } else {
            let annots_array = annot_ids.into_iter().map(Object::Reference).collect();
            page_dict.set("Annots", Object::Array(annots_array));
        }
    }
    Ok(())
}

fn create_link_annotation(
    doc: &mut LopdfDocument,
    source_page_id: ObjectId,
    target_page_id: ObjectId,
    rect: [f32; 4],
    page_height: f32,
    target_y: f32,
) -> Result<(), PipelineError> {
    let y_dest = page_height - target_y;
    let dest = vec![
        Object::Reference(target_page_id),
        "FitH".into(),
        y_dest.into(),
    ];
    let action = dictionary! { "Type" => "Action", "S" => "GoTo", "D" => dest };
    let action_id = doc.add_object(action);

    let pdf_rect = vec![
        rect[0].into(),
        (page_height - rect[3]).into(),
        rect[2].into(),
        (page_height - rect[1]).into(),
    ];

    let annot = dictionary! {
        "Type" => "Annot", "Subtype" => "Link", "Rect" => pdf_rect,
        "Border" => vec![0.into(), 0.into(), 0.into()], "A" => action_id,
    };
    let annot_id = doc.add_object(annot);

    append_annotations_to_page(doc, source_page_id, vec![annot_id])?;
    Ok(())
}

fn create_link_annotations_for_body(
    doc: &mut LopdfDocument,
    doc_meta: &Document,
    final_page_ids: &[ObjectId],
    page_height: f32,
    prepended_pages: usize,
) -> Result<HashMap<usize, Vec<ObjectId>>, PipelineError> {
    let mut annots_by_page: HashMap<usize, Vec<ObjectId>> = HashMap::new();
    let anchor_map: HashMap<String, &Anchor> =
        doc_meta.anchors.iter().map(|a| (a.id.clone(), a)).collect();

    for link in &doc_meta.hyperlinks {
        if let Some(anchor) = anchor_map.get(&link.target_id) {
            let adjusted_anchor_page = anchor.page_number + prepended_pages;
            if adjusted_anchor_page > 0 && adjusted_anchor_page <= final_page_ids.len() {
                let target_page_id = final_page_ids[adjusted_anchor_page - 1];
                let y_dest = page_height - anchor.y_position;
                let dest = vec![
                    Object::Reference(target_page_id),
                    "FitH".into(),
                    y_dest.into(),
                ];
                let action = dictionary! { "Type" => "Action", "S" => "GoTo", "D" => dest };
                let action_id = doc.add_object(action);
                let rect = vec![
                    link.rect[0].into(),
                    (page_height - link.rect[3]).into(),
                    link.rect[2].into(),
                    (page_height - link.rect[1]).into(),
                ];
                let annot = dictionary! {
                    "Type" => "Annot", "Subtype" => "Link", "Rect" => rect,
                    "Border" => vec![0.into(), 0.into(), 0.into()], "A" => action_id,
                };
                let annot_id = doc.add_object(annot);
                let adjusted_link_page = link.page_number + prepended_pages;
                annots_by_page
                    .entry(adjusted_link_page)
                    .or_default()
                    .push(annot_id);
            }
        }
    }
    Ok(annots_by_page)
}

fn build_outlines_for_doc(
    doc: &mut LopdfDocument,
    doc_meta: &Document,
    final_page_ids: &[ObjectId],
    page_height: f32,
    prepended_pages: usize,
) -> Result<Option<ObjectId>, PipelineError> {
    if doc_meta.headings.is_empty() {
        return Ok(None);
    }
    let anchor_map: std::collections::BTreeMap<String, &Anchor> =
        doc_meta.anchors.iter().map(|a| (a.id.clone(), a)).collect();

    struct FlatOutlineItem {
        id: ObjectId,
        parent_idx: Option<usize>,
        dict: lopdf::Dictionary,
    }
    struct NodeOutlineItem {
        id: ObjectId,
        children: Vec<NodeOutlineItem>,
        dict: lopdf::Dictionary,
    }

    let mut flat_list = Vec::new();
    let mut level_stack: Vec<(u8, usize)> = vec![(0, usize::MAX)];

    for entry in &doc_meta.headings {
        if let Some(anchor) = anchor_map.get(&entry.id) {
            let adjusted_page = anchor.page_number + prepended_pages;
            if adjusted_page == 0 || adjusted_page > final_page_ids.len() {
                continue;
            }
            let dest_page_id = final_page_ids[adjusted_page - 1];
            let y_dest = page_height - anchor.y_position;
            let dest = vec![
                Object::Reference(dest_page_id),
                "FitH".into(),
                y_dest.into(),
            ];

            while level_stack.last().unwrap().0 >= entry.level {
                level_stack.pop();
            }
            let parent_idx = if level_stack.last().unwrap().1 == usize::MAX {
                None
            } else {
                Some(level_stack.last().unwrap().1)
            };
            let dict = dictionary! { "Title" => Object::String(entry.text.as_bytes().to_vec(), StringFormat::Literal), "Dest" => dest };
            let new_item = FlatOutlineItem {
                id: doc.new_object_id(),
                parent_idx,
                dict,
            };
            let new_idx = flat_list.len();
            flat_list.push(new_item);
            level_stack.push((entry.level, new_idx));
        }
    }

    if flat_list.is_empty() {
        return Ok(None);
    }
    let mut children_map: HashMap<usize, Vec<NodeOutlineItem>> = HashMap::new();
    let mut root_items = Vec::new();
    for (i, flat_node) in flat_list.into_iter().enumerate().rev() {
        let mut children = children_map.remove(&i).unwrap_or_default();
        children.reverse();
        let node = NodeOutlineItem {
            id: flat_node.id,
            children,
            dict: flat_node.dict,
        };
        if let Some(parent_idx) = flat_node.parent_idx {
            children_map.entry(parent_idx).or_default().push(node);
        } else {
            root_items.push(node);
        }
    }
    root_items.reverse();
    if root_items.is_empty() {
        return Ok(None);
    }

    let outline_root_id = doc.add_object(dictionary! {
        "Type" => "Outlines",
        "First" => Object::Reference(root_items.first().unwrap().id),
        "Last" => Object::Reference(root_items.last().unwrap().id),
        "Count" => root_items.len() as i64,
    });

    fn add_outline_level(items: &[NodeOutlineItem], parent_id: ObjectId, doc: &mut LopdfDocument) {
        for (i, item) in items.iter().enumerate() {
            let mut dict = item.dict.clone();
            dict.set("Parent", Object::Reference(parent_id));
            if i > 0 {
                dict.set("Prev", Object::Reference(items[i - 1].id));
            }
            if i < items.len() - 1 {
                dict.set("Next", Object::Reference(items[i + 1].id));
            }
            if !item.children.is_empty() {
                dict.set(
                    "First",
                    Object::Reference(item.children.first().unwrap().id),
                );
                dict.set("Last", Object::Reference(item.children.last().unwrap().id));
                dict.set("Count", -(item.children.len() as i64));
                add_outline_level(&item.children, item.id, doc);
            }
            doc.objects.insert(item.id, dict.into());
        }
    }
    add_outline_level(&root_items, outline_root_id, doc);
    Ok(Some(outline_root_id))
}
