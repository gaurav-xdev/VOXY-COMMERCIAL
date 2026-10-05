use dioxus::prelude::*;

use crate::bridge::AppBridge;

#[component]
pub fn LoginView() -> Element {
    let bridge = use_context::<AppBridge>();
    let mut email = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut is_register_mode = use_signal(|| false);
    let mut status_message = use_signal(|| Option::<String>::None);
    let is_loading = use_signal(|| false);

    let session = bridge.auth.current_session();

    let on_submit = {
        let auth = bridge.auth.clone();
        move |_: Event<MouseData>| {
            let em = email.read().trim().to_string();
            let pass = password.read().to_string();
            let is_reg = *is_register_mode.read();
            let mut status = status_message;
            let mut loading = is_loading;
            let a = auth.clone();

            if em.is_empty() || pass.is_empty() {
                status.set(Some("Please enter both email and password".to_string()));
                return;
            }

            loading.set(true);
            status.set(None);

            spawn(async move {
                if is_reg {
                    match a.register(&em, &pass, None).await {
                        Ok(_) => {
                            // Auto login after register
                            match a.login(&em, &pass).await {
                                Ok(sess) => {
                                    status.set(Some(format!("Welcome, {}!", sess.email)));
                                }
                                Err(e) => {
                                    status.set(Some(format!(
                                        "Account created, but login failed: {e}"
                                    )));
                                }
                            }
                        }
                        Err(e) => {
                            status.set(Some(format!("Registration failed: {e}")));
                        }
                    }
                } else {
                    match a.login(&em, &pass).await {
                        Ok(sess) => {
                            status.set(Some(format!("Signed in as {}", sess.email)));
                        }
                        Err(e) => {
                            status.set(Some(format!("Sign in failed: {e}")));
                        }
                    }
                }
                loading.set(false);
            });
        }
    };

    let on_logout = {
        let auth = bridge.auth.clone();
        move |_: Event<MouseData>| {
            let a = auth.clone();
            let mut status = status_message;
            spawn(async move {
                let _ = a.logout().await;
                status.set(Some("Signed out successfully".to_string()));
            });
        }
    };

    rsx! {
        div {
            style: "display: flex; align-items: center; justify-content: center; min-height: 100%;",
            div { class: "card", style: "width: 420px; text-align: center; padding: 32px;",
                div { style: "font-size: 32px; font-weight: 700; color: var(--accent-secondary); margin-bottom: 8px;", "OSMOO" }

                if let Some(sess) = session {
                    div { style: "font-size: 13px; color: var(--text-muted); margin-bottom: 24px;", "Active Account Session" }
                    div { style: "margin-bottom: 16px; padding: 16px; background: var(--bg-tertiary); border-radius: var(--radius-md); text-align: left;",
                        div { style: "font-size: 12px; color: var(--text-muted);", "Signed in as" }
                        div { style: "font-size: 16px; font-weight: 600; color: var(--text-primary); margin-top: 4px;", "{sess.email}" }
                        div { style: "font-size: 12px; color: var(--accent-primary); margin-top: 8px;", "Tier: {sess.active_tier}" }
                    }
                    button {
                        class: "btn btn-secondary",
                        style: "width: 100%; margin-top: 16px;",
                        onclick: on_logout,
                        "Sign Out"
                    }
                } else {
                    div { style: "font-size: 13px; color: var(--text-muted); margin-bottom: 24px;",
                        if *is_register_mode.read() { "Create a new OSMOO account" } else { "Sign in to your account" }
                    }

                    div { style: "margin-bottom: 16px; text-align: left;",
                        input {
                            class: "input",
                            placeholder: "Email address",
                            r#type: "email",
                            value: "{email}",
                            oninput: move |e| email.set(e.value()),
                        }
                    }
                    div { style: "margin-bottom: 24px; text-align: left;",
                        input {
                            class: "input",
                            placeholder: "Password (min 8 characters)",
                            r#type: "password",
                            value: "{password}",
                            oninput: move |e| password.set(e.value()),
                        }
                    }

                    if let Some(msg) = status_message.read().as_ref() {
                        div { style: "color: var(--warning); margin-bottom: 16px; font-size: 12px;", "{msg}" }
                    }

                    button {
                        class: "btn btn-primary",
                        style: "width: 100%;",
                        disabled: *is_loading.read(),
                        onclick: on_submit,
                        if *is_loading.read() {
                            "Processing..."
                        } else if *is_register_mode.read() {
                            "Create Account"
                        } else {
                            "Sign In"
                        }
                    }

                    div { style: "margin-top: 16px; font-size: 12px; color: var(--text-muted);",
                        if *is_register_mode.read() {
                            "Already have an account? "
                        } else {
                            "Don't have an account? "
                        }
                        span {
                            style: "color: var(--accent-secondary); cursor: pointer; text-decoration: underline;",
                            onclick: move |_| {
                                let cur = *is_register_mode.read();
                                is_register_mode.set(!cur);
                                status_message.set(None);
                            },
                            if *is_register_mode.read() { "Sign in" } else { "Sign up" }
                        }
                    }
                }
            }
        }
    }
}
