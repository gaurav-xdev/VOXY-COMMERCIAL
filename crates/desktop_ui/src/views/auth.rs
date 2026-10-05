use dioxus::prelude::*;

use crate::bridge::AppBridge;

#[derive(Props, Clone, PartialEq)]
pub struct AuthViewProps {
    pub on_authenticated: EventHandler<()>,
}

#[component]
pub fn AuthView(props: AuthViewProps) -> Element {
    let bridge = use_context::<AppBridge>();
    let mut email = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut is_register_mode = use_signal(|| false);
    let mut status_message = use_signal(|| Option::<String>::None);
    let mut is_error = use_signal(|| false);
    let is_loading = use_signal(|| false);

    let session = bridge.auth.current_session();

    let on_submit = {
        let auth = bridge.auth.clone();
        let on_auth_handler = props.on_authenticated;
        move |_: Event<MouseData>| {
            let em = email.read().trim().to_string();
            let pass = password.read().to_string();
            let is_reg = *is_register_mode.read();
            let mut status = status_message;
            let mut error_flag = is_error;
            let mut loading = is_loading;
            let a = auth.clone();

            if em.is_empty() || pass.is_empty() {
                error_flag.set(true);
                status.set(Some("Email and password credentials are required.".to_string()));
                return;
            }

            loading.set(true);
            status.set(None);
            error_flag.set(false);

            let on_done = on_auth_handler;

            spawn(async move {
                if is_reg {
                    match a.register(&em, &pass, None).await {
                        Ok(_) => {
                            match a.login(&em, &pass).await {
                                Ok(sess) => {
                                    error_flag.set(false);
                                    status.set(Some(format!("Account created. Welcome, {}.", sess.email)));
                                    on_done.call(());
                                }
                                Err(e) => {
                                    error_flag.set(true);
                                    status.set(Some(format!("Registration succeeded, but initial sign-in failed: {e}")));
                                }
                            }
                        }
                        Err(e) => {
                            error_flag.set(true);
                            status.set(Some(format!("{e}")));
                        }
                    }
                } else {
                    match a.login(&em, &pass).await {
                        Ok(_) => {
                            error_flag.set(false);
                            on_done.call(());
                        }
                        Err(e) => {
                            error_flag.set(true);
                            status.set(Some(format!("{e}")));
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
            let mut error_flag = is_error;
            spawn(async move {
                let _ = a.logout().await;
                error_flag.set(false);
                status.set(Some("Terminated active session.".to_string()));
            });
        }
    };

    rsx! {
        div {
            class: "osmoo-auth-viewport",

            div { class: "osmoo-monolithic-panel",
                // Architectural Brand Header
                div { class: "panel-header-mark",
                    div { class: "glyph-dot" }
                    div { class: "glyph-title", "OSMOO" }
                    div { class: "glyph-subtitle", "OPERATING COMPANION" }
                }

                if let Some(sess) = session {
                    // Authenticated State Card
                    div { class: "auth-status-card",
                        div { class: "session-caption", "AUTHENTICATED PRINCIPAL" }
                        div { class: "session-principal", "{sess.email}" }
                        div { class: "session-badge", "TIER // {sess.active_tier.to_uppercase()}" }
                    }

                    button {
                        class: "btn btn-secondary",
                        style: "width: 100%; margin-top: 18px;",
                        onclick: on_logout,
                        "Disconnect Session"
                    }
                } else {
                    // Clean Functional Auth Form
                    div { class: "auth-mode-indicator",
                        if *is_register_mode.read() {
                            "CREATE LOCAL PRINCIPAL ACCOUNT"
                        } else {
                            "ENTER COMPUTATIONAL ENVIRONMENT"
                        }
                    }

                    div { class: "field-group",
                        div { class: "field-label", "IDENTIFIER" }
                        input {
                            class: "input",
                            placeholder: "name@domain.com",
                            r#type: "email",
                            value: "{email}",
                            oninput: move |e| email.set(e.value()),
                        }
                    }

                    div { class: "field-group",
                        div { class: "field-label", "CREDENTIAL" }
                        input {
                            class: "input",
                            placeholder: "••••••••••••",
                            r#type: "password",
                            value: "{password}",
                            oninput: move |e| password.set(e.value()),
                        }
                    }

                    if let Some(msg) = status_message.read().as_ref() {
                        div {
                            class: if *is_error.read() { "feedback-msg error" } else { "feedback-msg success" },
                            "{msg}"
                        }
                    }

                    button {
                        class: "btn btn-primary",
                        style: "width: 100%; margin-top: 8px;",
                        disabled: *is_loading.read(),
                        onclick: on_submit,
                        if *is_loading.read() {
                            "VERIFYING..."
                        } else if *is_register_mode.read() {
                            "INITIALIZE ACCOUNT"
                        } else {
                            "AUTHENTICATE"
                        }
                    }

                    div { class: "auth-toggle-row",
                        if *is_register_mode.read() {
                            "Existing credentials? "
                        } else {
                            "Need principal access? "
                        }
                        span {
                            class: "auth-toggle-btn",
                            onclick: move |_| {
                                let cur = *is_register_mode.read();
                                is_register_mode.set(!cur);
                                status_message.set(None);
                                is_error.set(false);
                            },
                            if *is_register_mode.read() { "Authenticate" } else { "Create Account" }
                        }
                    }
                }

                // Parent Organization Subtle Footnote
                div { class: "panel-parent-footnote",
                    "ENGINEERED BY OSMIORA // OSMOO.IN"
                }
            }
        }
    }
}
