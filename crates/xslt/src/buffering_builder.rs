use crate::ast::PreparsedStyles;
use crate::output::OutputBuilder;
use petty_style::dimension::Dimension;

#[derive(Clone)]
enum OutputCommand {
    StartBlock(PreparsedStyles),
    EndBlock,
    StartFlexContainer(PreparsedStyles),
    EndFlexContainer,
    StartParagraph(PreparsedStyles),
    EndParagraph,
    StartList(PreparsedStyles),
    EndList,
    StartListItem(PreparsedStyles),
    EndListItem,
    StartImage(PreparsedStyles),
    EndImage,
    StartTable(PreparsedStyles),
    EndTable,
    StartTableHeader,
    EndTableHeader,
    SetTableColumns(Vec<Dimension>),
    StartTableRow(PreparsedStyles),
    EndTableRow,
    StartTableCell(PreparsedStyles),
    EndTableCell,
    AddText(String),
    StartHeading(PreparsedStyles, u8),
    EndHeading,
    AddPageBreak(Option<String>),
    StartStyledSpan(PreparsedStyles),
    EndStyledSpan,
    StartHyperlink(PreparsedStyles),
    EndHyperlink,
    SetAttribute(String, String),
}

pub struct BufferingOutputBuilder<'a> {
    target: &'a mut dyn OutputBuilder,
    buffer: Vec<OutputCommand>,
    buffering: bool,
}

impl<'a> BufferingOutputBuilder<'a> {
    pub fn new(target: &'a mut dyn OutputBuilder) -> Self {
        Self {
            target,
            buffer: Vec::new(),
            buffering: false,
        }
    }

    pub fn start_buffering(&mut self) {
        self.buffering = true;
        self.buffer.clear();
    }

    pub fn flush(&mut self) {
        for cmd in self.buffer.drain(..) {
            Self::apply_command(self.target, cmd);
        }
        self.buffering = false;
    }

    pub fn discard(&mut self) {
        self.buffer.clear();
        self.buffering = false;
    }

    pub fn target_mut(&mut self) -> &mut dyn OutputBuilder {
        self.target
    }

    pub fn has_buffered_content(&self) -> bool {
        !self.buffer.is_empty()
    }

    pub fn flush_if_populated(&mut self) {
        if !self.buffer.is_empty() {
            self.flush();
        } else {
            self.buffering = false;
        }
    }

    fn apply_command(target: &mut dyn OutputBuilder, cmd: OutputCommand) {
        match cmd {
            OutputCommand::StartBlock(s) => target.start_block(&s),
            OutputCommand::EndBlock => target.end_block(),
            OutputCommand::StartFlexContainer(s) => target.start_flex_container(&s),
            OutputCommand::EndFlexContainer => target.end_flex_container(),
            OutputCommand::StartParagraph(s) => target.start_paragraph(&s),
            OutputCommand::EndParagraph => target.end_paragraph(),
            OutputCommand::StartList(s) => target.start_list(&s),
            OutputCommand::EndList => target.end_list(),
            OutputCommand::StartListItem(s) => target.start_list_item(&s),
            OutputCommand::EndListItem => target.end_list_item(),
            OutputCommand::StartImage(s) => target.start_image(&s),
            OutputCommand::EndImage => target.end_image(),
            OutputCommand::StartTable(s) => target.start_table(&s),
            OutputCommand::EndTable => target.end_table(),
            OutputCommand::StartTableHeader => target.start_table_header(),
            OutputCommand::EndTableHeader => target.end_table_header(),
            OutputCommand::SetTableColumns(cols) => target.set_table_columns(&cols),
            OutputCommand::StartTableRow(s) => target.start_table_row(&s),
            OutputCommand::EndTableRow => target.end_table_row(),
            OutputCommand::StartTableCell(s) => target.start_table_cell(&s),
            OutputCommand::EndTableCell => target.end_table_cell(),
            OutputCommand::AddText(t) => target.add_text(&t),
            OutputCommand::StartHeading(s, l) => target.start_heading(&s, l),
            OutputCommand::EndHeading => target.end_heading(),
            OutputCommand::AddPageBreak(m) => target.add_page_break(m),
            OutputCommand::StartStyledSpan(s) => target.start_styled_span(&s),
            OutputCommand::EndStyledSpan => target.end_styled_span(),
            OutputCommand::StartHyperlink(s) => target.start_hyperlink(&s),
            OutputCommand::EndHyperlink => target.end_hyperlink(),
            OutputCommand::SetAttribute(n, v) => target.set_attribute(&n, &v),
        }
    }

    fn record_or_apply(&mut self, cmd: OutputCommand) {
        if self.buffering {
            self.buffer.push(cmd);
        } else {
            Self::apply_command(self.target, cmd);
        }
    }
}

impl OutputBuilder for BufferingOutputBuilder<'_> {
    fn start_block(&mut self, styles: &PreparsedStyles) {
        self.record_or_apply(OutputCommand::StartBlock(styles.clone()));
    }
    fn end_block(&mut self) {
        self.record_or_apply(OutputCommand::EndBlock);
    }
    fn start_flex_container(&mut self, styles: &PreparsedStyles) {
        self.record_or_apply(OutputCommand::StartFlexContainer(styles.clone()));
    }
    fn end_flex_container(&mut self) {
        self.record_or_apply(OutputCommand::EndFlexContainer);
    }
    fn start_paragraph(&mut self, styles: &PreparsedStyles) {
        self.record_or_apply(OutputCommand::StartParagraph(styles.clone()));
    }
    fn end_paragraph(&mut self) {
        self.record_or_apply(OutputCommand::EndParagraph);
    }
    fn start_list(&mut self, styles: &PreparsedStyles) {
        self.record_or_apply(OutputCommand::StartList(styles.clone()));
    }
    fn end_list(&mut self) {
        self.record_or_apply(OutputCommand::EndList);
    }
    fn start_list_item(&mut self, styles: &PreparsedStyles) {
        self.record_or_apply(OutputCommand::StartListItem(styles.clone()));
    }
    fn end_list_item(&mut self) {
        self.record_or_apply(OutputCommand::EndListItem);
    }
    fn start_image(&mut self, styles: &PreparsedStyles) {
        self.record_or_apply(OutputCommand::StartImage(styles.clone()));
    }
    fn end_image(&mut self) {
        self.record_or_apply(OutputCommand::EndImage);
    }
    fn start_table(&mut self, styles: &PreparsedStyles) {
        self.record_or_apply(OutputCommand::StartTable(styles.clone()));
    }
    fn end_table(&mut self) {
        self.record_or_apply(OutputCommand::EndTable);
    }
    fn start_table_header(&mut self) {
        self.record_or_apply(OutputCommand::StartTableHeader);
    }
    fn end_table_header(&mut self) {
        self.record_or_apply(OutputCommand::EndTableHeader);
    }
    fn set_table_columns(&mut self, columns: &[Dimension]) {
        self.record_or_apply(OutputCommand::SetTableColumns(columns.to_vec()));
    }
    fn start_table_row(&mut self, styles: &PreparsedStyles) {
        self.record_or_apply(OutputCommand::StartTableRow(styles.clone()));
    }
    fn end_table_row(&mut self) {
        self.record_or_apply(OutputCommand::EndTableRow);
    }
    fn start_table_cell(&mut self, styles: &PreparsedStyles) {
        self.record_or_apply(OutputCommand::StartTableCell(styles.clone()));
    }
    fn end_table_cell(&mut self) {
        self.record_or_apply(OutputCommand::EndTableCell);
    }
    fn add_text(&mut self, text: &str) {
        self.record_or_apply(OutputCommand::AddText(text.to_string()));
    }
    fn start_heading(&mut self, styles: &PreparsedStyles, level: u8) {
        self.record_or_apply(OutputCommand::StartHeading(styles.clone(), level));
    }
    fn end_heading(&mut self) {
        self.record_or_apply(OutputCommand::EndHeading);
    }
    fn add_page_break(&mut self, master_name: Option<String>) {
        self.record_or_apply(OutputCommand::AddPageBreak(master_name));
    }
    fn start_styled_span(&mut self, styles: &PreparsedStyles) {
        self.record_or_apply(OutputCommand::StartStyledSpan(styles.clone()));
    }
    fn end_styled_span(&mut self) {
        self.record_or_apply(OutputCommand::EndStyledSpan);
    }
    fn start_hyperlink(&mut self, styles: &PreparsedStyles) {
        self.record_or_apply(OutputCommand::StartHyperlink(styles.clone()));
    }
    fn end_hyperlink(&mut self) {
        self.record_or_apply(OutputCommand::EndHyperlink);
    }
    fn set_attribute(&mut self, name: &str, value: &str) {
        self.record_or_apply(OutputCommand::SetAttribute(
            name.to_string(),
            value.to_string(),
        ));
    }
}
