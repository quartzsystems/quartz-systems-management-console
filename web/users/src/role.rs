// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Roles, tasks and the matrix between them, as the prototype's Roles
//! page has them (`console-roles.html`). The matrix is state: an
//! Administrator changes it on that page. Administrator always has
//! every task; its column is not kept.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A web user's role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Every task, always.
    Administrator,
    /// Operations.
    Operator,
    /// Looking.
    Viewer,
    /// Service.
    Service,
}

/// The roles in the matrix's order.
pub const ROLES: [Role; 4] = [
    Role::Administrator,
    Role::Operator,
    Role::Viewer,
    Role::Service,
];

impl Role {
    /// Its name on the pages.
    pub fn label(self) -> &'static str {
        match self {
            Role::Administrator => "Administrator",
            Role::Operator => "Operator",
            Role::Viewer => "Viewer",
            Role::Service => "Service",
        }
    }

    /// The role named `label` or its lower-case key.
    pub fn parse(s: &str) -> Option<Role> {
        ROLES
            .into_iter()
            .find(|r| r.label().eq_ignore_ascii_case(s.trim()))
    }

    /// Its column in the matrix.
    pub fn column(self) -> usize {
        ROLES.iter().position(|r| *r == self).unwrap_or(0)
    }
}

/// A task a route needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Task {
    /// View systems and partitions.
    View,
    /// Open terminal window.
    Terminal,
    /// Activate / shut down partitions.
    Activate,
    /// Power on / off managed systems.
    Power,
    /// Create / delete partitions.
    CreatePartitions,
    /// Virtual resources.
    VirtualResources,
    /// System plans.
    Plans,
    /// Serviceable events.
    Events,
    /// Code updates.
    CodeUpdates,
    /// Console settings.
    Settings,
    /// Users and roles.
    Users,
    /// Control panel functions 50-70.
    Panel50,
}

/// The tasks in the matrix's order.
pub const TASKS: [Task; 12] = [
    Task::View,
    Task::Terminal,
    Task::Activate,
    Task::Power,
    Task::CreatePartitions,
    Task::VirtualResources,
    Task::Plans,
    Task::Events,
    Task::CodeUpdates,
    Task::Settings,
    Task::Users,
    Task::Panel50,
];

impl Task {
    /// Its row's text on the Roles page.
    pub fn label(self) -> &'static str {
        match self {
            Task::View => "View systems and partitions",
            Task::Terminal => "Open terminal window",
            Task::Activate => "Activate / shut down partitions",
            Task::Power => "Power on / off managed systems",
            Task::CreatePartitions => "Create / delete partitions",
            Task::VirtualResources => "Virtual resources",
            Task::Plans => "System plans",
            Task::Events => "Serviceable events",
            Task::CodeUpdates => "Code updates",
            Task::Settings => "Console settings",
            Task::Users => "Users and roles",
            Task::Panel50 => "Control panel functions 50-70",
        }
    }

    /// Its row in the matrix.
    pub fn row(self) -> usize {
        TASKS.iter().position(|t| *t == self).unwrap_or(0)
    }
}

/// Which roles have which tasks. Administrator's column is always full
/// and not kept.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Matrix {
    /// The tasks of each role but Administrator.
    pub grants: BTreeMap<Role, Vec<Task>>,
}

impl Default for Matrix {
    /// The prototype's matrix.
    fn default() -> Self {
        use Task::{CodeUpdates, Events, Panel50, Power, Terminal, View};
        let mut grants = BTreeMap::new();
        grants.insert(
            Role::Operator,
            vec![View, Terminal, Task::Activate, Power, Events],
        );
        grants.insert(Role::Viewer, vec![View, Events]);
        grants.insert(
            Role::Service,
            vec![View, Terminal, Events, CodeUpdates, Panel50],
        );
        Self { grants }
    }
}

impl Matrix {
    /// Whether `role` has `task`.
    pub fn allows(&self, role: Role, task: Task) -> bool {
        role == Role::Administrator || self.grants.get(&role).is_some_and(|t| t.contains(&task))
    }

    /// Give or take `task` of `role`. Administrator's are not changed.
    pub fn set(&mut self, role: Role, task: Task, on: bool) {
        if role == Role::Administrator {
            return;
        }
        let tasks = self.grants.entry(role).or_default();
        tasks.retain(|t| *t != task);
        if on {
            tasks.push(task);
            tasks.sort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_the_prototype_s() {
        let m = Matrix::default();
        // Rows as `console-roles.html` checks them, columns A O V S.
        let want = [
            "XXXX", "XX-X", "XX--", "XX--", "X---", "X---", "X---", "XXXX", "X--X", "X---", "X---",
            "X--X",
        ];
        for (task, row) in TASKS.iter().zip(want) {
            let got: String = ROLES
                .iter()
                .map(|r| if m.allows(*r, *task) { 'X' } else { '-' })
                .collect();
            assert_eq!(got, row, "{}", task.label());
        }
    }

    #[test]
    fn administrator_keeps_every_task() {
        let mut m = Matrix::default();
        m.set(Role::Administrator, Task::Users, false);
        assert!(m.allows(Role::Administrator, Task::Users));
        m.set(Role::Viewer, Task::Settings, true);
        assert!(m.allows(Role::Viewer, Task::Settings));
        m.set(Role::Viewer, Task::Settings, false);
        assert!(!m.allows(Role::Viewer, Task::Settings));
        assert_eq!(Role::parse("operator"), Some(Role::Operator));
    }
}
