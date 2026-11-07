//! Component slots: one file per component that reads named tokens (`list.row`,
//! `sidebar.row`, `tabs.tab`, …). Slots are API (plan K4).
//!
//! Work package: W2, `docs/work/phase-0-groundwork/W2-look-contract.md`. Each component's work
//! package fills in its own slot file. W0 declared the slot files the work package docs name,
//! so their owners never edit this file; a new slot file is declared here in its own PR.

pub mod icons;
pub mod list;
pub mod sidebar;
pub mod tabs;
