// src/pipeline/renderer/composing.rs
use crate::core::layout::LayoutEngine;
use crate::error::PipelineError;
use crate::parser::processor::{DataSourceFormat, ExecutionConfig};
use crate::pipeline::api::{Anchor, Document, PreparedDataSources};
use crate::pipeline::context::PipelineContext;
use crate::pipeline::renderer::RenderingStrategy;
use crate::render::composer::{merge_documents, overlay_content};
use crate::render::lopdf_renderer::LopdfRenderer;
use crate::render::DocumentRenderer as _;
use log::{info, warn};
use lopdf::{dictionary, Document as LopdfDocument, Object, ObjectId, StringFormat};
use serde_json::json;
use std::collections::{BTreeMap, HashMap};
use std::io::{Cursor, Seek, Write};

/// A rendering strategy that composes a final document from multiple sources.
///
/// This renderer is the counterpart to the `MetadataGeneratingProvider`. It expects
/// to receive a `Document` object and a pre-rendered PDF body. Its primary jobs are:
///
/// 1.  **Composition:** Execute "role templates" (e.g., for a table of contents)
///     using the `Document` object as a data source, render them to separate PDF pages,
///     and merge them with the main PDF body.
/// 2.  **Overlays:** Execute "overlay" templates (e.g., for page headers/footers)
///     and apply their content to each page of the final document.
/// 3.  **Fixups:** Apply document-wide modifications that were impossible in a
///     single pass, such as adding hyperlink annotations and PDF outlines (bookmarks),
///     using the metadata from the `Document` object.
#[derive(Clone)]
pub struct ComposingRenderer;

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
        info!("[COMPOSER] Body artifact loaded with {} pages.", main_doc.get_pages().len());

        let stylesheet = context.compiled_template.stylesheet();
        let (page_width, page_height) = stylesheet.get_default_page_layout().size.dimensions_pt();
        let mut prepended_pages = 0;

        // --- Phase 1: Page Generation & Merging (for roles like ToC) ---
        let page_generating_roles = ["cover-page", "preface", "table-of-contents", "back-cover"];
        for (role, template) in context.role_templates.iter() {
            if page_generating_roles.contains(&role.as_str()) {
                info!("[COMPOSER] Executing page-generating role template: '{}'", role);
                let doc_json_str = serde_json::to_string(&*doc_metadata)?;
                let exec_config = ExecutionConfig { format: DataSourceFormat::Json, strict: false };
                let ir_nodes = template.execute(&doc_json_str, exec_config)?;

                let layout_engine = LayoutEngine::new(context.font_manager.clone());
                let mut temp_renderer = LopdfRenderer::new(layout_engine, stylesheet.clone())?;
                temp_renderer.begin_document(Cursor::new(Vec::new()))?;
                let (laid_out_pages, _, _) =
                    temp_renderer.layout_engine.paginate(&stylesheet, ir_nodes)?;

                let mut new_page_ids = vec![];
                let font_map: HashMap<_, _> = temp_renderer.layout_engine.font_manager.db().faces().enumerate().map(|(i, f)| (f.post_script_name.clone(), format!("F{}", i + 1))).collect();

                for page_elements in laid_out_pages {
                    let content_id = temp_renderer.render_page_content(page_elements, &font_map, page_width, page_height)?;
                    let page_id = temp_renderer.write_page_object(vec![content_id], vec![], page_width, page_height)?;
                    new_page_ids.push(page_id);
                }

                let role_pdf_bytes = Box::new(temp_renderer).finish_into_buffer(new_page_ids)?;
                let role_doc = LopdfDocument::load_mem(&role_pdf_bytes)?;
                let prepend = role.as_str() != "back-cover";
                info!("[COMPOSER] Merging {} pages from role '{}' (prepend: {})", role_doc.get_pages().len(), role, prepend);

                if prepend { prepended_pages += role_doc.get_pages().len(); }
                merge_documents(&mut main_doc, role_doc, prepend)?;
            }
        }

        // --- Phase 2: Overlays (for roles like headers/footers) ---
        let final_page_count = main_doc.get_pages().len();
        let overlay_roles = ["page-header", "page-footer"];
        let page_ids: Vec<ObjectId> = main_doc.get_pages().into_values().collect();

        for (role, template) in context.role_templates.iter() {
            if overlay_roles.contains(&role.as_str()) {
                info!("[COMPOSER] Executing overlay role template: '{}'", role);
                let layout_engine = LayoutEngine::new(context.font_manager.clone());

                for (i, page_id) in page_ids.iter().enumerate() {
                    let page_number = i + 1;
                    let overlay_context_val = json!({
                        "document": &*doc_metadata, "page_number": page_number, "page_count": final_page_count
                    });
                    let overlay_context_str = serde_json::to_string(&overlay_context_val)?;
                    let exec_config = ExecutionConfig { format: DataSourceFormat::Json, strict: false };
                    let ir_nodes = template.execute(&overlay_context_str, exec_config)?;

                    let (mut overlay_pages, _, _) = layout_engine.paginate(&stylesheet, ir_nodes)?;
                    if let Some(elements) = overlay_pages.pop() {
                        if !overlay_pages.is_empty() {
                            warn!("[COMPOSER] Overlay template for role '{}' generated more than one page of content. Only the first will be used.", role);
                        }
                        let font_map: HashMap<_, _> = context.font_manager.db().faces().enumerate().map(|(i, f)| (f.post_script_name.clone(), format!("F{}", i + 1))).collect();
                        let content = crate::render::lopdf_helpers::render_elements_to_content(elements, &font_map, page_width, page_height)?;
                        overlay_content(&mut main_doc, *page_id, content.encode()?)?;
                    }
                }
            }
        }

        // --- Phase 3: Fixups (Links and Outlines) ---
        if !doc_metadata.hyperlinks.is_empty() || !doc_metadata.headings.is_empty() {
            info!("[COMPOSER] Applying fixups (links, outlines) to the document.");
            let final_page_ids: Vec<ObjectId> = main_doc.get_pages().into_values().collect();
            let root_id = main_doc.trailer.get(b"Root")?.as_reference()?;

            let annots_by_page = create_link_annotations_for_doc(&mut main_doc, &doc_metadata, &final_page_ids, page_height, prepended_pages)?;
            for (page_idx, annot_ids) in annots_by_page {
                if page_idx > 0 && page_idx <= final_page_ids.len() {
                    let page_id = final_page_ids[page_idx - 1];
                    if let Ok(Object::Dictionary(page_dict)) = main_doc.get_object_mut(page_id) {
                        let annots_array = annot_ids.into_iter().map(Object::Reference).collect();
                        page_dict.set("Annots", Object::Array(annots_array));
                    }
                }
            }

            if let Some(outline_root_id) = build_outlines_for_doc(&mut main_doc, &doc_metadata, &final_page_ids, page_height, prepended_pages)? {
                if let Ok(Object::Dictionary(root_dict_mut)) = main_doc.get_object_mut(root_id) {
                    root_dict_mut.set("Outlines", outline_root_id);
                    root_dict_mut.set("PageMode", "UseOutlines");
                }
            }
        } else {
            info!("[COMPOSER] No fixups required. Passing document through.");
        }

        main_doc.save_to(&mut writer)?;
        info!("[COMPOSER] Composition complete. Final document saved.");

        Ok(writer)
    }
}

/// Creates hyperlink annotations by consuming the `Document` metadata.
fn create_link_annotations_for_doc(
    doc: &mut LopdfDocument,
    doc_meta: &Document,
    final_page_ids: &[ObjectId],
    page_height: f32,
    prepended_pages: usize,
) -> Result<HashMap<usize, Vec<ObjectId>>, PipelineError> {
    let mut annots_by_page: HashMap<usize, Vec<ObjectId>> = HashMap::new();
    let anchor_map: HashMap<String, &Anchor> = doc_meta.anchors.iter().map(|a| (a.id.clone(), a)).collect();

    for link in &doc_meta.hyperlinks {
        if let Some(anchor) = anchor_map.get(&link.target_id) {
            // Adjust the target page number by the number of pages we prepended
            let adjusted_anchor_page = anchor.page_number + prepended_pages;
            if adjusted_anchor_page > 0 && adjusted_anchor_page <= final_page_ids.len() {
                let target_page_id = final_page_ids[adjusted_anchor_page - 1];
                let y_dest = page_height - anchor.y_position;
                let dest = vec![Object::Reference(target_page_id), "FitH".into(), y_dest.into()];
                let action = dictionary! { "Type" => "Action", "S" => "GoTo", "D" => dest };
                let action_id = doc.add_object(action);
                let rect = vec![
                    link.rect[0].into(), (page_height - link.rect[3]).into(),
                    link.rect[2].into(), (page_height - link.rect[1]).into(),
                ];
                let annot = dictionary! {
                    "Type" => "Annot", "Subtype" => "Link", "Rect" => rect,
                    "Border" => vec![0.into(), 0.into(), 0.into()], "A" => action_id,
                };
                let annot_id = doc.add_object(annot);
                // Adjust the page where the link annotation itself is placed
                let adjusted_link_page = link.page_number + prepended_pages;
                annots_by_page.entry(adjusted_link_page).or_default().push(annot_id);
            }
        } else {
            warn!("Hyperlink references non-existent anchor '{}'", link.target_id);
        }
    }
    Ok(annots_by_page)
}

/// Creates PDF outlines (bookmarks) by consuming the `Document` metadata.
fn build_outlines_for_doc(
    doc: &mut LopdfDocument,
    doc_meta: &Document,
    final_page_ids: &[ObjectId],
    page_height: f32,
    prepended_pages: usize,
) -> Result<Option<ObjectId>, PipelineError> {
    if doc_meta.headings.is_empty() { return Ok(None); }
    let anchor_map: BTreeMap<String, &Anchor> = doc_meta.anchors.iter().map(|a| (a.id.clone(), a)).collect();

    struct FlatOutlineItem { id: ObjectId, parent_idx: Option<usize>, dict: lopdf::Dictionary }
    struct NodeOutlineItem { id: ObjectId, children: Vec<NodeOutlineItem>, dict: lopdf::Dictionary }

    let mut flat_list = Vec::new();
    let mut level_stack: Vec<(u8, usize)> = vec![(0, usize::MAX)];

    for entry in &doc_meta.headings {
        if let Some(anchor) = anchor_map.get(&entry.id) {
            let adjusted_page = anchor.page_number + prepended_pages;
            if adjusted_page == 0 || adjusted_page > final_page_ids.len() { continue; }
            let dest_page_id = final_page_ids[adjusted_page - 1];
            let y_dest = page_height - anchor.y_position;
            let dest = vec![Object::Reference(dest_page_id), "FitH".into(), y_dest.into()];

            while level_stack.last().unwrap().0 >= entry.level { level_stack.pop(); }
            let parent_idx = if level_stack.last().unwrap().1 == usize::MAX { None } else { Some(level_stack.last().unwrap().1) };
            let dict = dictionary! { "Title" => Object::String(entry.text.as_bytes().to_vec(), StringFormat::Literal), "Dest" => dest };
            let new_item = FlatOutlineItem { id: doc.new_object_id(), parent_idx, dict, };
            let new_idx = flat_list.len();
            flat_list.push(new_item);
            level_stack.push((entry.level, new_idx));
        }
    }

    if flat_list.is_empty() { return Ok(None); }
    let mut children_map: HashMap<usize, Vec<NodeOutlineItem>> = HashMap::new();
    let mut root_items = Vec::new();
    for (i, flat_node) in flat_list.into_iter().enumerate().rev() {
        let mut children = children_map.remove(&i).unwrap_or_default();
        children.reverse();
        let node = NodeOutlineItem { id: flat_node.id, children, dict: flat_node.dict };
        if let Some(parent_idx) = flat_node.parent_idx { children_map.entry(parent_idx).or_default().push(node); }
        else { root_items.push(node); }
    }
    root_items.reverse();
    if root_items.is_empty() { return Ok(None); }

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
            if i > 0 { dict.set("Prev", Object::Reference(items[i - 1].id)); }
            if i < items.len() - 1 { dict.set("Next", Object::Reference(items[i + 1].id)); }
            if !item.children.is_empty() {
                dict.set("First", Object::Reference(item.children.first().unwrap().id));
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