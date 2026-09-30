//! Semantic UI published from the same state that drives Ratatui.

use super::{App, Ask, Focus, LeftView, Line, ListingViewRef, RefView};
use ::molerat::{
    accesskit::{Action, ActionData, Role},
    Molerat, Ui,
};
use ratatui::crossterm::event::{KeyEvent, KeyEventKind, MouseEvent};

#[derive(Debug)]
pub(super) enum Message {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Focus(Focus),
    ShowLeft(LeftView),
    SelectFunction(u64),
    SelectSink(usize),
    SelectDriver(usize),
    SelectType(usize),
    SelectBrowser(usize),
    SelectListing { left: bool, index: usize },
    SelectReference(usize),
    OpenFunction(u64),
    OpenSink(usize),
    OpenDriver(usize),
    OpenType(usize),
    OpenBrowser(usize),
    OpenBrowserRecord(usize),
    OpenBrowserDisassembly(usize),
    OpenListing { left: bool, index: usize },
    OpenReference(usize),
    OpenPrompt(Ask),
    SetPromptValue(Option<String>),
    SubmitPrompt,
    CancelPrompt,
    ExecuteCommand(&'static str),
    ToggleReachable,
    ToggleSeverity,
    Reanalyze,
    Quit,
}

impl App {
    /// Apply local terminal input or a semantic action on the TUI thread.
    pub(super) fn update(&mut self, message: Message) -> bool {
        if self.is_reloading()
            && !matches!(
                message,
                Message::Key(_) | Message::Quit | Message::CancelPrompt
            )
        {
            return false;
        }

        match message {
            Message::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    return false;
                }
                self.handle_key(key);
            }
            Message::Mouse(mouse) => self.handle_mouse(mouse),
            Message::Focus(focus) => {
                if focus == Focus::Xrefs && self.split {
                    self.status = "references are hidden while listings are split".into();
                    return true;
                }
                self.focus = focus;
            }
            Message::ShowLeft(view) => {
                self.browser = None;
                self.left = view;
                self.focus = Focus::Functions;
                match view {
                    LeftView::Functions => self.refilter(),
                    LeftView::Sinks => {
                        self.ssel = self.ssel.min(self.sinks.len().saturating_sub(1));
                    }
                    LeftView::Driver => self.clamp_dsel(),
                    LeftView::Types => self.clamp_tsel(),
                }
            }
            Message::SelectFunction(address) => {
                let Some(index) = self
                    .order
                    .iter()
                    .position(|&index| self.an.functions[index].addr == address)
                else {
                    return false;
                };
                self.sel = index;
                self.focus = Focus::Functions;
            }
            Message::SelectSink(index) => {
                if index >= self.sinks.len() {
                    return false;
                }
                self.ssel = index;
                self.focus = Focus::Functions;
            }
            Message::SelectDriver(index) => {
                if self.driver_rows().get(index).is_none_or(|row| row.section) {
                    return false;
                }
                self.dsel = index;
                self.focus = Focus::Functions;
            }
            Message::SelectType(index) => {
                if self.type_rows().get(index).is_none_or(|row| row.section) {
                    return false;
                }
                self.tsel = index;
                self.focus = Focus::Functions;
            }
            Message::SelectBrowser(index) => {
                let Some(browser) = &mut self.browser else {
                    return false;
                };
                if index >= browser.visible.len() {
                    return false;
                }
                browser.selection = index;
                self.focus = Focus::Functions;
            }
            Message::SelectListing { left, index } => {
                if self.split && left != self.active_is_left {
                    self.swap_views();
                }
                if index >= self.listing_len() {
                    return false;
                }
                self.cursor = index;
                self.focus = Focus::Listing;
            }
            Message::SelectReference(index) => {
                if self.split || index >= self.xref_rows().len() {
                    return false;
                }
                self.xsel = index;
                self.focus = Focus::Xrefs;
            }
            Message::OpenFunction(address) => {
                if !self.update(Message::SelectFunction(address)) {
                    return false;
                }
                self.open(address, true);
                self.focus = Focus::Listing;
            }
            Message::OpenSink(index) => {
                if !self.update(Message::SelectSink(index)) {
                    return false;
                }
                self.open_sink();
            }
            Message::OpenDriver(index) => {
                if !self.update(Message::SelectDriver(index)) {
                    return false;
                }
                self.open_driver();
            }
            Message::OpenType(index) => {
                if !self.update(Message::SelectType(index)) {
                    return false;
                }
                self.open_type();
            }
            Message::OpenBrowser(index) => {
                if !self.update(Message::SelectBrowser(index)) {
                    return false;
                }
                self.open_browser_selection();
            }
            Message::OpenBrowserRecord(index) => {
                if !self.update(Message::SelectBrowser(index)) {
                    return false;
                }
                self.open_browser_record();
            }
            Message::OpenBrowserDisassembly(index) => {
                if !self.update(Message::SelectBrowser(index)) {
                    return false;
                }
                self.open_browser_disassembly();
            }
            Message::OpenListing { left, index } => {
                if !self.update(Message::SelectListing { left, index }) {
                    return false;
                }
                if self.graph {
                    self.open_graph_block();
                } else {
                    self.follow();
                }
            }
            Message::OpenReference(index) => {
                if !self.update(Message::SelectReference(index)) {
                    return false;
                }
                self.jump_xref();
            }
            Message::OpenPrompt(ask) => self.ask(ask),
            Message::SetPromptValue(Some(value)) => {
                let Some(prompt) = &mut self.prompt else {
                    return false;
                };
                prompt.input = value;
                if prompt.ask == Ask::Filter {
                    let value = prompt.input.clone();
                    self.set_filter(value);
                }
            }
            Message::SetPromptValue(None) => return false,
            Message::SubmitPrompt => {
                let Some(prompt) = self.prompt.take() else {
                    return false;
                };
                self.commit_with_context(
                    prompt.ask,
                    prompt.at,
                    prompt.input,
                    prompt.field,
                    prompt.variable,
                );
            }
            Message::CancelPrompt => {
                if self.prompt.take().is_none() && !self.help {
                    return false;
                }
                self.help = false;
            }
            Message::ExecuteCommand(command) => self.execute_command(command),
            Message::ToggleReachable => {
                self.dreach = !self.dreach;
                self.clamp_dsel();
                self.status = format!(
                    "driver pane: {} reachable primitives",
                    if self.dreach { "only" } else { "all" }
                );
            }
            Message::ToggleSeverity => {
                self.dminsev = if self.dminsev >= 3 { 1 } else { 3 };
                self.clamp_dsel();
                self.status = format!(
                    "driver pane: severity {}",
                    if self.dminsev >= 3 { ">= 3" } else { "any" }
                );
            }
            Message::Reanalyze => {
                self.reanalyze();
                self.status = "re-analysed".into();
            }
            Message::Quit => self.quit = true,
        }
        true
    }

    /// Publish one complete semantic snapshot of the visible workstation.
    pub(super) fn publish(&self, molerat: &mut Molerat<Message>) {
        molerat.publish(|ui| {
            ui.group("target", "Binary target", |ui| {
                ui.text("name", &self.title);
                ui.text("format", format!("Format: {}", self.bin.format.label()));
                ui.text(
                    "architecture",
                    format!("Architecture: {}", self.bin.arch.label()),
                );
                ui.text(
                    "summary",
                    format!(
                        "{} recovered functions; {} high-risk findings",
                        self.an.functions.len(),
                        self.sinks
                            .iter()
                            .filter(|finding| finding.severity >= 3)
                            .count()
                    ),
                );
            });

            if let Some(prompt) = &self.prompt {
                ui.group("prompt", "Active prompt", |ui| {
                    ui.text_input("input", prompt.ask.label(), &prompt.input)
                        .description(prompt_description(prompt.ask))
                        .focused(true)
                        .on_action(Action::SetValue, |request| {
                            Message::SetPromptValue(match request.data.as_ref() {
                                Some(ActionData::Value(value)) => Some(value.to_string()),
                                _ => None,
                            })
                        });
                    ui.button("submit", "Submit")
                        .on_click(|| Message::SubmitPrompt);
                    ui.button("cancel", "Cancel")
                        .on_click(|| Message::CancelPrompt);
                });
                return;
            }
            if self.help {
                ui.command("close-help", "Close help", || Message::CancelPrompt);
                ui.paragraph("help", help_text())
                    .label("Keyboard and command help")
                    .focused(true)
                    .custom_action("close", "Close help", || Message::CancelPrompt);
                return;
            }
            if self.is_reloading() {
                ui.command("quit", "Quit Knife", || Message::Quit);
                ui.status("operation-status", "Reloading the target from disk");
                return;
            }

            publish_commands(ui, self);
            publish_left_pane(ui, self);
            if self.split {
                let (left, right) = if self.active_is_left {
                    (self.active_view(), self.other_view())
                } else {
                    (self.other_view(), self.active_view())
                };
                publish_listing(ui, self, "listing-left", "Left listing", left, true);
                publish_listing(ui, self, "listing-right", "Right listing", right, false);
            } else {
                publish_listing(ui, self, "listing", "Listing", self.active_view(), true);
                publish_references(ui, self);
            }

            if !self.status.is_empty() {
                ui.status("operation-status", &self.status);
            }
        });
    }
}

fn publish_commands(ui: &mut Ui<'_, Message>, app: &App) {
    ui.command("command", "Open the command prompt", || {
        Message::OpenPrompt(Ask::Command)
    });
    ui.command("focus-functions", "Move focus to the left pane", || {
        Message::Focus(Focus::Functions)
    });
    ui.command("focus-listing", "Move focus to the active listing", || {
        Message::Focus(Focus::Listing)
    });
    if !app.split {
        ui.command("focus-references", "Move focus to references", || {
            Message::Focus(Focus::Xrefs)
        });
    }
    ui.command("goto", "Go to an address or symbol", || {
        Message::OpenPrompt(Ask::Goto)
    });
    ui.command("filter", "Filter the active left pane", || {
        Message::OpenPrompt(Ask::Filter)
    });
    ui.command("search", "Search within the current listing", || {
        Message::OpenPrompt(Ask::Search)
    });
    ui.command("rename", "Rename the current function", || {
        Message::OpenPrompt(Ask::Name)
    });
    ui.command("comment", "Edit the note at the listing cursor", || {
        Message::OpenPrompt(Ask::Note)
    });
    ui.command("back", "Return to the previous navigation location", || {
        Message::ExecuteCommand("back")
    });
    ui.command("forward", "Move forward in navigation history", || {
        Message::ExecuteCommand("forward")
    });
    ui.command("next-function", "Open the next recovered function", || {
        Message::ExecuteCommand("next")
    });
    ui.command(
        "previous-function",
        "Open the previous recovered function",
        || Message::ExecuteCommand("previous"),
    );
    ui.command(
        "bookmark",
        "Toggle a bookmark at the listing cursor",
        || Message::ExecuteCommand("bookmark"),
    );
    ui.command("disassemble", "Show the linear disassembly", || {
        Message::ExecuteCommand("disasm")
    });
    ui.command(
        "decompile",
        "Show pseudocode for the current function",
        || Message::ExecuteCommand("decompile"),
    );
    ui.command(
        "cfg",
        "Show the current function control-flow graph",
        || Message::ExecuteCommand("cfg"),
    );
    if app.split {
        ui.command("only", "Close the inactive listing column", || {
            Message::ExecuteCommand("only")
        });
    } else {
        ui.command("xrefs", "Show references to the selected address", || {
            Message::ExecuteCommand("xrefs")
        });
        ui.command("callers", "Show callers of the current function", || {
            Message::ExecuteCommand("callers")
        });
        ui.command("callees", "Show callees of the current function", || {
            Message::ExecuteCommand("callees")
        });
        ui.command("split", "Clone the listing into two columns", || {
            Message::ExecuteCommand("split")
        });
    }
    ui.command(
        "compare",
        "Compare the current function in a second column",
        || Message::ExecuteCommand("compare"),
    );
    ui.command("zoom", "Toggle zoom for the focused pane", || {
        Message::ExecuteCommand("zoom")
    });
    ui.command("close-pane", "Close the focused side pane", || {
        Message::ExecuteCommand("close")
    });
    ui.command("reload", "Reload the target from disk", || {
        Message::ExecuteCommand("reload")
    });
    ui.command("reanalyze", "Re-run analysis on the current bytes", || {
        Message::Reanalyze
    });
    ui.command("help", "Show keyboard and command help", || {
        Message::ExecuteCommand("help")
    });
    ui.command("quit", "Quit Knife", || Message::Quit);

    if app.pseudo {
        ui.command(
            "bind-type",
            "Bind the selected pseudocode field base to a type",
            || Message::OpenPrompt(Ask::Type),
        );
        ui.command("name-field", "Name the selected pseudocode field", || {
            Message::OpenPrompt(Ask::Field)
        });
        ui.command(
            "edit-prototype",
            "Edit the current function prototype",
            || Message::OpenPrompt(Ask::Prototype),
        );
        ui.command(
            "rename-variable",
            "Rename the selected recovered variable",
            || Message::OpenPrompt(Ask::Variable),
        );
    } else if !app.graph && matches!(app.lines.get(app.cursor), Some(Line::Insn { .. })) {
        ui.command("patch", "Stage bytes at the selected instruction", || {
            Message::OpenPrompt(Ask::Patch)
        });
    }
}

fn publish_left_pane(ui: &mut Ui<'_, Message>, app: &App) {
    ui.container("left-views", Role::TabList, |ui| {
        for (view, key, label) in [
            (LeftView::Functions, "functions", "Functions"),
            (LeftView::Sinks, "findings", "Risk findings"),
            (LeftView::Driver, "driver", "Driver surface"),
            (LeftView::Types, "types", "Analyst types"),
        ] {
            ui.node(key, Role::Tab)
                .label(label)
                .selected(app.browser.is_none() && app.left == view)
                .on_click(move || Message::ShowLeft(view));
        }
    })
    .label("Left pane views");

    if let Some(browser) = &app.browser {
        ui.list_box("catalog", browser.catalog.title(), |ui| {
            for (position, &row_index) in browser.visible.iter().enumerate() {
                let row = &browser.rows[row_index];
                let option = ui
                    .list_box_option(
                        format!("{row_index}:{}", row_key(row)),
                        format!("{} {}", row.label, row.detail),
                    )
                    .selected(position == browser.selection)
                    .focused(
                        app.prompt.is_none()
                            && !app.help
                            && app.focus == Focus::Functions
                            && position == browser.selection,
                    )
                    .on_focus(move || Message::SelectBrowser(position))
                    .on_click(move || Message::SelectBrowser(position))
                    .custom_action("open", "Open this catalog entry", move || {
                        Message::OpenBrowser(position)
                    });
                if browser.catalog.is_elf_structure() {
                    option
                        .custom_action(
                            "open-record",
                            "Open the on-disk structure record",
                            move || Message::OpenBrowserRecord(position),
                        )
                        .custom_action(
                            "open-disassembly",
                            "Open the mapped disassembly",
                            move || Message::OpenBrowserDisassembly(position),
                        );
                }
            }
        });
        return;
    }

    match app.left {
        LeftView::Functions => {
            ui.list_box("functions", "Recovered functions", |ui| {
                for (position, &function_index) in app.order.iter().enumerate() {
                    let function = &app.an.functions[function_index];
                    let address = function.addr;
                    ui.list_box_option(
                        format!("{address:x}"),
                        format!("0x{address:x} {}", function.name),
                    )
                    .description(format!(
                        "{} bytes; {} incoming references",
                        function.size, function.incoming
                    ))
                    .selected(position == app.sel)
                    .focused(left_row_focused(app, position == app.sel))
                    .on_focus(move || Message::SelectFunction(address))
                    .on_click(move || Message::SelectFunction(address))
                    .custom_action(
                        "open",
                        "Open this function in the listing",
                        move || Message::OpenFunction(address),
                    );
                }
            });
        }
        LeftView::Sinks => {
            ui.list_box("findings", "Risk findings", |ui| {
                for (index, finding) in app.sinks.iter().enumerate() {
                    ui.list_box_option(
                        format!("{:x}:{}:{index}", finding.addr, finding.pattern),
                        format!(
                            "Severity {} {} at 0x{:x}",
                            finding.severity, finding.api, finding.addr
                        ),
                    )
                    .description(format!(
                        "{}; function {}; {}",
                        finding.detail,
                        finding.func.as_deref().unwrap_or("unknown"),
                        if finding.reachable {
                            "reachable"
                        } else {
                            "not known reachable"
                        }
                    ))
                    .selected(index == app.ssel)
                    .focused(left_row_focused(app, index == app.ssel))
                    .on_focus(move || Message::SelectSink(index))
                    .on_click(move || Message::SelectSink(index))
                    .custom_action(
                        "open",
                        "Open this finding's call site",
                        move || Message::OpenSink(index),
                    );
                }
            });
        }
        LeftView::Driver => {
            ui.group("driver-controls", "Driver filters", |ui| {
                ui.checkbox("reachable", "Only reachable primitives", app.dreach)
                    .on_click(|| Message::ToggleReachable);
                ui.checkbox("critical", "Only severity 3 or higher", app.dminsev >= 3)
                    .on_click(|| Message::ToggleSeverity);
            });
            let rows = app.driver_rows();
            ui.list_box("driver", "Kernel driver surface", |ui| {
                for (index, row) in rows.iter().enumerate() {
                    ui.list_box_option(
                        row.addr
                            .map(|address| format!("{address:x}:{index}"))
                            .unwrap_or_else(|| format!("section:{index}")),
                        format!("{} {}", row.label, row.detail),
                    )
                    .disabled(row.section || row.addr.is_none())
                    .selected(!row.section && index == app.dsel)
                    .focused(!row.section && left_row_focused(app, index == app.dsel))
                    .on_focus(move || Message::SelectDriver(index))
                    .on_click(move || Message::SelectDriver(index))
                    .custom_action(
                        "open",
                        "Open this driver item",
                        move || Message::OpenDriver(index),
                    );
                }
            });
        }
        LeftView::Types => {
            ui.group("type-library", "Type library operations", |ui| {
                ui.button("import", "Import type library")
                    .on_click(|| Message::OpenPrompt(Ask::ImportLibrary));
                ui.button("replace", "Import and replace colliding types")
                    .on_click(|| Message::OpenPrompt(Ask::ReplaceLibrary));
                ui.button("export", "Export type library")
                    .on_click(|| Message::OpenPrompt(Ask::ExportLibrary));
            });
            let rows = app.type_rows();
            ui.list_box("types", "Analyst types and prototypes", |ui| {
                for (index, row) in rows.iter().enumerate() {
                    ui.list_box_option(
                        format!("{}:{}:{index}", row.kind, row.addr.unwrap_or_default()),
                        format!("{} {}", row.label, row.detail),
                    )
                    .disabled(row.section)
                    .selected(!row.section && index == app.tsel)
                    .focused(!row.section && left_row_focused(app, index == app.tsel))
                    .on_focus(move || Message::SelectType(index))
                    .on_click(move || Message::SelectType(index))
                    .custom_action("open", "Open this type fact", move || {
                        Message::OpenType(index)
                    });
                }
            });
        }
    };
}

fn publish_listing(
    ui: &mut Ui<'_, Message>,
    app: &App,
    key: &str,
    label: &str,
    view: ListingViewRef<'_>,
    left: bool,
) {
    let mode = if view.graph {
        "control-flow graph"
    } else if view.pseudo {
        "pseudocode"
    } else if view.raw_file {
        "raw file bytes"
    } else {
        "disassembly"
    };
    ui.list_box(key, format!("{label}: {mode}"), |ui| {
        for index in 0..listing_len(app, &view) {
            let (row_key, row_label, description) = listing_row(app, &view, index);
            ui.list_box_option(row_key, row_label)
                .description(description)
                .selected(index == view.cursor)
                .focused(app.prompt.is_none() && !app.help && view.focused && index == view.cursor)
                .on_focus(move || Message::SelectListing { left, index })
                .on_click(move || Message::SelectListing { left, index })
                .custom_action("follow", "Follow or open this listing item", move || {
                    Message::OpenListing { left, index }
                });
        }
    })
    .description(format!(
        "Current location: {}",
        view.cur
            .map(|address| format!("0x{address:x}"))
            .unwrap_or_else(|| "none".into())
    ));
}

fn publish_references(ui: &mut Ui<'_, Message>, app: &App) {
    let rows = app.xref_rows();
    let label = match app.refview {
        RefView::To => "References to selected address",
        RefView::Callers => "Callers of current function",
        RefView::From => "Callees of current function",
    };
    ui.list_box("references", label, |ui| {
        for (index, row) in rows.iter().enumerate() {
            ui.list_box_option(
                format!("{:x}:{:x}:{}:{index}", row.site, row.jump, row.kind),
                format!("0x{:x} {} {}", row.site, row.kind, row.label),
            )
            .description(format!("Jump target 0x{:x}", row.jump))
            .selected(index == app.xsel)
            .focused(
                app.prompt.is_none() && !app.help && app.focus == Focus::Xrefs && index == app.xsel,
            )
            .on_focus(move || Message::SelectReference(index))
            .on_click(move || Message::SelectReference(index))
            .custom_action("open", "Open this reference", move || {
                Message::OpenReference(index)
            });
        }
    });
}

fn listing_len(app: &App, view: &ListingViewRef<'_>) -> usize {
    if view.graph {
        view.cur
            .and_then(|address| app.an.find_function(address))
            .map(|function| function.blocks.len())
            .unwrap_or(0)
    } else if view.pseudo {
        view.pseudo_lines.len()
    } else {
        view.lines.len()
    }
}

fn listing_row(app: &App, view: &ListingViewRef<'_>, index: usize) -> (String, String, String) {
    if view.graph {
        if let Some(block) = view
            .cur
            .and_then(|address| app.an.find_function(address))
            .and_then(|function| function.blocks.get(index))
        {
            let address = block.start + app.base;
            let successors = block
                .succ
                .iter()
                .map(|address| format!("0x{:x}", address + app.base))
                .collect::<Vec<_>>()
                .join(", ");
            let instructions = block
                .insns
                .iter()
                .map(|instruction| instruction.text(app.an.bits, app.an.arch))
                .collect::<Vec<_>>()
                .join("; ");
            return (
                format!("block:{address:x}"),
                format!("Basic block 0x{address:x}"),
                format!("Instructions: {instructions}. Successors: {successors}"),
            );
        }
    } else if view.pseudo {
        if let Some(line) = view.pseudo_lines.get(index) {
            return (
                format!("pseudo:{index}"),
                line.text.clone(),
                if line.label {
                    "Pseudocode label".into()
                } else {
                    "Pseudocode statement".into()
                },
            );
        }
    } else if let Some(line) = view.lines.get(index) {
        let address = line.addr();
        return match line {
            Line::Label { text, .. } => (
                format!("label:{address:x}:{index}"),
                format!("0x{address:x} {text}"),
                "Disassembly label".into(),
            ),
            Line::Insn {
                mnemonic,
                operands,
                annot,
                target,
                ..
            } => (
                format!("instruction:{address:x}:{index}"),
                format!("0x{address:x} {mnemonic} {operands}"),
                format!(
                    "{}{}",
                    annot
                        .as_ref()
                        .map(|annotation| format!("Annotation: {annotation:?}. "))
                        .unwrap_or_default(),
                    target
                        .map(|target| format!("Target: 0x{target:x}"))
                        .unwrap_or_else(|| "No follow target".into())
                ),
            ),
            Line::Data { text, .. } => (
                format!("data:{address:x}:{index}"),
                format!("0x{address:x} {text}"),
                "Data bytes".into(),
            ),
        };
    }
    (
        format!("missing:{index}"),
        "Unavailable listing row".into(),
        String::new(),
    )
}

fn left_row_focused(app: &App, selected: bool) -> bool {
    app.prompt.is_none() && !app.help && app.focus == Focus::Functions && selected
}

fn row_key(row: &super::browser::Row) -> String {
    if let Some(address) = row.address {
        format!("va:{:x}", address.get())
    } else if let Some(offset) = row.file_offset {
        format!("off:{:x}", offset.get())
    } else {
        row.label.clone()
    }
}

fn prompt_description(ask: Ask) -> &'static str {
    match ask {
        Ask::Command => "Enter a Knife command without the leading colon",
        Ask::Filter => "Filter the active functions, catalog, driver, or types view",
        Ask::Name => "Set the current function name; empty clears it",
        Ask::Note => "Set the note at the listing cursor; empty clears it",
        Ask::Goto => "Enter a symbol, static VA, va: address, or off: file offset",
        Ask::Search => "Find the next matching listing row",
        Ask::Type => "Bind the selected pseudocode base to a type",
        Ask::Field => "Name the selected field in its bound type",
        Ask::Prototype => "Enter RETURN_TYPE (PARAM_TYPE, ...)",
        Ask::Variable => "Rename the selected recovered variable",
        Ask::Patch => "Enter replacement hex bytes; empty restores the original bytes",
        Ask::ImportLibrary => "Enter the type-library file to merge",
        Ask::ReplaceLibrary => "Enter the type-library file and replace collisions",
        Ask::ExportLibrary => "Enter the type-library export path",
    }
}

fn help_text() -> &'static str {
    "Navigate selectable lists directly or use the semantic commands. Important keyboard equivalents: Enter opens or follows; Tab changes pane; / filters or searches; g goes to an address; n renames; c comments; b bookmarks; d shows pseudocode; f shows the CFG; x changes reference direction; s cycles left views; r re-analyses; : opens the command prompt; q quits."
}
