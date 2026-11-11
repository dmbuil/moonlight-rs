//! Orchestration stacks (Heat).

use super::*;

/// The *Stacks* list's bulk menu over the selection (keys from [`stack_key`]).
pub fn bulk_stack_items(lang: Lang, picked: Vec<(String, String)>) -> Vec<MenuItem> {
    let item = |label: K, make: fn(String) -> OsAction| {
        let p = picked.clone();
        MenuItem::dialog(t(lang, label), move || bulk(lang, label, p.clone(), make, false))
    };
    vec![
        item(K::CheckStacks, |k| {
            let (name, id) = split_stack_key(&k);
            OsAction::StackCheck { name, id }
        }),
        item(K::SuspendStacks, |k| {
            let (name, id) = split_stack_key(&k);
            OsAction::StackSuspend { name, id }
        }),
        item(K::ResumeStacks, |k| {
            let (name, id) = split_stack_key(&k);
            OsAction::StackResume { name, id }
        }),
    ]
}

#[cfg(test)]
mod stack_tests {
    use super::*;

    fn row(status: &str) -> StackRow {
        StackRow {
            id: "s1".into(),
            name: "web".into(),
            description: None,
            status: status.into(),
            status_reason: None,
            created: String::new(),
            updated: None,
        }
    }

    #[test]
    fn stack_menu_follows_the_status() {
        let l = Lang::EnUs;
        let labels = |s: &str| stack_items(l, &row(s)).into_iter().map(|i| i.label).collect::<Vec<_>>();
        assert_eq!(
            labels("CREATE_COMPLETE"),
            ["Change Stack Template", "Check Stack", "Suspend Stack", "Delete Stack"]
        );
        assert_eq!(
            labels("SUSPEND_COMPLETE"),
            ["Change Stack Template", "Check Stack", "Resume Stack", "Delete Stack"]
        );
        assert_eq!(
            labels("UPDATE_IN_PROGRESS"),
            ["Delete Stack"],
            "Heat refuses actions while busy"
        );
    }

    #[test]
    fn stack_keys_round_trip() {
        assert_eq!(split_stack_key(&stack_key("web", "s1")), ("web".into(), "s1".into()));
        assert_eq!(split_stack_key("broken"), (String::new(), String::new()));
    }
}
