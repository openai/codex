//! Groups the onboarding flow screens used during first-run and login setup.
//!
//! The module provides the building blocks for onboarding UI flows such as the
//! welcome screen, authentication choices, and directory trust prompts. It is
//! responsible for wiring together these screens, while the concrete widgets
//! own their own state and rendering logic.

/// Authentication widgets and state machines for onboarding sign-in flows.
mod auth;

/// Coordinates the onboarding steps into a single screen-level flow.
pub mod onboarding_screen;

/// Directory trust prompt UI and selections for onboarding.
mod trust_directory;

/// Re-exports the trust decision used by onboarding callers.
pub use trust_directory::TrustDirectorySelection;

/// Introductory welcome screen UI shown at the start of onboarding.
mod welcome;
