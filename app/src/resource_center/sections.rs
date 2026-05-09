use yarp_core::{context_flag::ContextFlag, features::FeatureFlag};
use yarpui::ViewContext;

use super::{
    ContentItem, ContentSectionData, FeatureItem, FeatureSection, FeatureSectionData,
    ResourceCenterMainView, Section, Tip, TipAction, TipHint,
};

pub fn sections(ctx: &mut ViewContext<ResourceCenterMainView>) -> Vec<Section> {
    let mut sections = vec![Section::Changelog()];

    if FeatureFlag::AvatarInTabBar.is_enabled() {
        return sections;
    }

    let get_started = FeatureSectionData {
        section_name: FeatureSection::GettingStarted,
        items: vec![
            FeatureItem::new(
                "Log your first case file",
                "Run a command — Yarp bundles the order and the response into one file.",
                Tip::Hint(TipHint::CreateBlock),
                ctx,
            ),
            FeatureItem::new(
                "Walk the case files",
                "Click one and step through the rest with the arrow keys.",
                Tip::Hint(TipHint::BlockSelect),
                ctx,
            ),
            FeatureItem::new(
                "Act on a case file",
                "Right-click for copy, paste, share, and the rest of the kit.",
                Tip::Hint(TipHint::BlockAction),
                ctx,
            ),
            FeatureItem::new(
                "Call up the command palette",
                "Radio in any Yarp action from the keyboard.",
                Tip::Action(TipAction::CommandPalette),
                ctx,
            ),
            FeatureItem::new(
                "Set your uniform",
                "Pick a theme that suits the station.",
                Tip::Action(TipAction::ThemePicker),
                ctx,
            ),
        ],
    };
    sections.push(Section::Feature(get_started));

    let maximize_yarp = FeatureSectionData {
        section_name: FeatureSection::MaximizeYarp,
        items: maximize_yarp_items(ctx),
    };
    sections.push(Section::Feature(maximize_yarp));

    let advanced_setup = ContentSectionData {
        section_name: FeatureSection::AdvancedSetup,
        items: vec![
            ContentItem {
                title: "Bring your own prompt",
                description: "Sign Yarp on to honor your PS1 setting.",
                url: "https://github.com/hotfuzz/yarp/terminal/appearance/prompt",
                button_label: "Standing Orders",
            },
            ContentItem {
                title: "Patch Yarp into your IDE",
                description: "Wire Yarp up to launch straight from your kit of choice.",
                url: "https://github.com/hotfuzz/yarp/terminal/integrations-and-plugins",
                button_label: "Standing Orders",
            },
            ContentItem {
                title: "How the force runs Yarp",
                description: "How Yarp's own squad runs the kit.",
                url: "https://github.com/hotfuzz/yarp/blog/how-yarp-uses-yarp",
                button_label: "Read the file",
            },
        ],
    };
    sections.push(Section::Content(advanced_setup));

    sections
}

fn maximize_yarp_items(ctx: &mut ViewContext<ResourceCenterMainView>) -> Vec<FeatureItem> {
    let mut maximize_yarp_items = vec![];

    maximize_yarp_items.push(FeatureItem::new(
        "Sweep the records",
        "Pull up old commands, playbooks, and the rest of the case file.",
        Tip::Action(TipAction::CommandSearch),
        ctx,
    ));

    maximize_yarp_items.push(FeatureItem::new(
        "Radio for a command",
        "Spell it out in plain English — the PC writes the shell command.",
        Tip::Action(TipAction::AiCommandSearch),
        ctx,
    ));

    if ContextFlag::CreateNewSession.is_enabled() {
        maximize_yarp_items.push(FeatureItem::new(
            "Split the beat",
            "Carve a beat into panes — patrol multiple sessions side by side.",
            Tip::Action(TipAction::SplitPane),
            ctx,
        ));
    }

    if ContextFlag::LaunchConfigurations.is_enabled() {
        maximize_yarp_items.push(FeatureItem::new(
            "Save the duty roster",
            "Snapshot your windows, beats, and panes — pin them for the next shift.",
            Tip::Action(TipAction::SaveNewLaunchConfig),
            ctx,
        ));
    }

    maximize_yarp_items
}
