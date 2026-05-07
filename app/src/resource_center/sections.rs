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
                "Run a command to see your command and output bundled together.",
                Tip::Hint(TipHint::CreateBlock),
                ctx,
            ),
            FeatureItem::new(
                "Walk the case files",
                "Click to pick one and step through with arrow keys.",
                Tip::Hint(TipHint::BlockSelect),
                ctx,
            ),
            FeatureItem::new(
                "Act on a case file",
                "Right click on a case file for copy, paste, share, and more.",
                Tip::Hint(TipHint::BlockAction),
                ctx,
            ),
            FeatureItem::new(
                "Call up the command palette",
                "Run any Yarp action from the keyboard.",
                Tip::Action(TipAction::CommandPalette),
                ctx,
            ),
            FeatureItem::new(
                "Set your uniform",
                "Pick a theme that suits your station.",
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
                title: "Use your custom prompt",
                description: "Set up Yarp to honor your PS1 setting",
                url: "https://github.com/hotfuzz/yarp/terminal/appearance/prompt",
                button_label: "View documentation",
            },
            ContentItem {
                title: "Integrate Yarp with your IDE",
                description: "Configure Yarp to launch from your most used development tools",
                url: "https://github.com/hotfuzz/yarp/terminal/integrations-and-plugins",
                button_label: "View documentation",
            },
            ContentItem {
                title: "How Yarp uses Yarp",
                description: "Learn how Yarp's engineering team uses their favorite features",
                url: "https://github.com/hotfuzz/yarp/blog/how-yarp-uses-yarp",
                button_label: "Read article",
            },
        ],
    };
    sections.push(Section::Content(advanced_setup));

    sections
}

fn maximize_yarp_items(ctx: &mut ViewContext<ResourceCenterMainView>) -> Vec<FeatureItem> {
    let mut maximize_yarp_items = vec![];

    maximize_yarp_items.push(FeatureItem::new(
        "Search the records",
        "Find and re-run previous commands, workflows, and more.",
        Tip::Action(TipAction::CommandSearch),
        ctx,
    ));

    maximize_yarp_items.push(FeatureItem::new(
        "Radio for a command",
        "Spell it out in plain English; the PC writes the shell command.",
        Tip::Action(TipAction::AiCommandSearch),
        ctx,
    ));

    if ContextFlag::CreateNewSession.is_enabled() {
        maximize_yarp_items.push(FeatureItem::new(
            "Split the beat",
            "Split tabs into multiple panes to lay out your station.",
            Tip::Action(TipAction::SplitPane),
            ctx,
        ));
    }

    if ContextFlag::LaunchConfigurations.is_enabled() {
        maximize_yarp_items.push(FeatureItem::new(
            "Save the duty roster",
            "Snapshot your current windows, tabs, and panes for next shift.",
            Tip::Action(TipAction::SaveNewLaunchConfig),
            ctx,
        ));
    }

    maximize_yarp_items
}
