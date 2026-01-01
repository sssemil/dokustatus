//! Public domain authentication routes.
//!
//! This module provides authentication and billing routes for end-users
//! of domains using the reauth platform.
//!
//! # Route Groups
//!
//! - **Config** (1 route): Domain configuration
//! - **Magic Link** (2 routes): Email-based authentication
//! - **Session** (4 routes): Session management, logout, account deletion
//! - **Google OAuth** (5 routes): Google sign-in integration
//! - **Billing** (9 routes): Plans, subscriptions, checkout, payments
//! - **Billing Webhooks** (2 routes): Stripe webhook handlers
//! - **Billing Dummy** (3 routes): Test payment provider

mod billing;
mod billing_dummy;
mod billing_webhooks;
mod common;
mod config;
mod google_oauth;
mod magic_link;
mod session;

use crate::adapters::http::app_state::AppState;
use axum::Router;

/// Returns the combined router for all public domain auth routes.
pub fn router() -> Router<AppState> {
    config::router()
        .merge(magic_link::router())
        .merge(session::router())
        .merge(google_oauth::router())
        .merge(billing::router())
        .merge(billing_webhooks::router())
        .merge(billing_dummy::router())
}
