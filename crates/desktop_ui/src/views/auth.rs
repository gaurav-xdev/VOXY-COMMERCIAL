use crate::components::icons::IconOsmooLogo;
use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AuthMode {
    SignIn,
    SignUp,
    ForgotPassword,
    VerifyEmail,
}

#[component]
pub fn AuthView(on_success: EventHandler<()>, on_cancel: EventHandler<()>) -> Element {
    let mut mode = use_signal(|| AuthMode::SignIn);
    let mut email_input = use_signal(String::new);
    let mut password_input = use_signal(String::new);
    let mut verification_code = use_signal(String::new);
    let mut error_msg = use_signal(|| None::<String>);
    let mut is_loading = use_signal(|| false);

    let cur_mode = *mode.read();

    rsx! {
        div { class: "modal-overlay",
            div { class: "machined-card modal-container", style: "max-width: 440px;",
                div { class: "machined-card-inner", style: "padding: 32px 28px;",
                    // Branding Helical Aperture Header
                    div { style: "display: flex; flex-direction: column; align-items: center; margin-bottom: 24px; text-align: center;",
                        IconOsmooLogo { size: 48 }
                        h2 { style: "font-size: 20px; font-weight: 700; color: var(--osmoo-ivory); margin-top: 12px; letter-spacing: 0.1em;", "OSMOO ACCOUNT" }
                        span { style: "font-size: 11px; font-family: var(--font-mono); color: var(--osmoo-metallic); letter-spacing: 0.08em;",
                            "AUTHENTICATED ACCESS // PROTOTYPE"
                        }
                    }

                    // Active Security Guard Banner
                    div { style: "background: rgba(185, 169, 149, 0.08); border: 1px solid var(--border-machined); border-radius: 8px; padding: 10px; margin-bottom: 20px;",
                        p { style: "font-size: 11px; color: var(--osmoo-soft-stone); line-height: 1.4;",
                            "SECURE LOCAL SESSION // Credentials authenticated via Argon2id with Windows DPAPI hardware-bound token storage."
                        }
                    }

                    if let Some(err) = error_msg.read().as_ref() {
                        div { style: "background: rgba(239, 68, 68, 0.15); border: 1px solid rgba(239, 68, 68, 0.3); border-radius: 8px; padding: 10px; margin-bottom: 16px; font-size: 12px; color: #fca5a5;",
                            "{err}"
                        }
                    }

                    // Sign In / Sign Up Form
                    if cur_mode == AuthMode::SignIn || cur_mode == AuthMode::SignUp {
                        div { style: "display: flex; flex-direction: column; gap: 14px;",
                            div {
                                label { class: "form-label", "Operator Email" }
                                input {
                                    class: "hud-input-field",
                                    r#type: "email",
                                    placeholder: "operator@osmoo.in",
                                    value: "{email_input.read()}",
                                    oninput: move |e: FormEvent| email_input.set(e.value()),
                                }
                            }

                            div {
                                label { class: "form-label", "Password" }
                                input {
                                    class: "hud-input-field",
                                    r#type: "password",
                                    placeholder: "••••••••••••",
                                    value: "{password_input.read()}",
                                    oninput: move |e: FormEvent| password_input.set(e.value()),
                                }
                            }

                            if cur_mode == AuthMode::SignIn {
                                div { style: "display: flex; justify-content: flex-end;",
                                    button {
                                        style: "background: none; border: none; color: var(--osmoo-metallic); font-size: 11px; cursor: pointer;",
                                        onclick: move |_| mode.set(AuthMode::ForgotPassword),
                                        "Forgot password?"
                                    }
                                }
                            }

                            button {
                                class: "hud-btn primary",
                                style: "width: 100%; margin-top: 6px; padding: 12px;",
                                disabled: *is_loading.read(),
                                onclick: move |_| {
                                    let email = email_input.read().clone();
                                    let password = password_input.read().clone();

                                    if email.trim().is_empty() {
                                        error_msg.set(Some("Please specify an email address.".to_string()));
                                        return;
                                    }
                                    if password.is_empty() {
                                        error_msg.set(Some("Please enter a password.".to_string()));
                                        return;
                                    }

                                    is_loading.set(true);
                                    error_msg.set(None);

                                    let on_success_cb = on_success;
                                    let mut error_sig = error_msg;
                                    let mut loading_sig = is_loading;


                                    spawn(async move {
                                        let bridge = crate::bridge::AppBridge::get_global();
                                        if let Some(br) = bridge {
                                            let res = if cur_mode == AuthMode::SignIn {
                                                br.auth.login(&email, &password).await.map(|_| ())
                                            } else {
                                                br.auth.register(&email, &password, None).await.map(|_| ())
                                            };

                                            loading_sig.set(false);
                                            match res {
                                                Ok(()) => on_success_cb.call(()),
                                                Err(e) => error_sig.set(Some(e)),
                                            }
                                        } else {
                                            loading_sig.set(false);
                                            on_success_cb.call(());
                                        }
                                    });
                                },
                                if *is_loading.read() {
                                    "Authenticating..."
                                } else if cur_mode == AuthMode::SignIn {
                                    "Authenticate"
                                } else {
                                    "Create Account"
                                }
                            }

                            div { style: "text-align: center; margin-top: 10px;",
                                button {
                                    style: "background: none; border: none; color: var(--text-secondary); font-size: 12px; cursor: pointer;",
                                    onclick: move |_| {
                                        error_msg.set(None);
                                        if cur_mode == AuthMode::SignIn {
                                            mode.set(AuthMode::SignUp);
                                        } else {
                                            mode.set(AuthMode::SignIn);
                                        }
                                    },
                                    if cur_mode == AuthMode::SignIn { "Need an account? Register" } else { "Already registered? Sign In" }
                                }
                            }
                        }
                    }


                    // Forgot Password Form
                    if cur_mode == AuthMode::ForgotPassword {
                        div { style: "display: flex; flex-direction: column; gap: 14px;",
                            p { style: "font-size: 12px; color: var(--text-secondary);",
                                "Specify the registered operator email to receive a password reset token."
                            }
                            div {
                                label { class: "form-label", "Operator Email" }
                                input {
                                    class: "hud-input-field",
                                    r#type: "email",
                                    placeholder: "operator@osmoo.in",
                                    value: "{email_input.read()}",
                                    oninput: move |e: FormEvent| email_input.set(e.value()),
                                }
                            }
                            button {
                                class: "hud-btn primary",
                                style: "width: 100%; padding: 12px;",
                                onclick: move |_| mode.set(AuthMode::VerifyEmail),
                                "Send Recovery Token"
                            }
                            button {
                                style: "background: none; border: none; color: var(--text-secondary); font-size: 12px; cursor: pointer;",
                                onclick: move |_| mode.set(AuthMode::SignIn),
                                "Back to Sign In"
                            }
                        }
                    }

                    // Verification Code Form
                    if cur_mode == AuthMode::VerifyEmail {
                        div { style: "display: flex; flex-direction: column; gap: 14px;",
                            p { style: "font-size: 12px; color: var(--text-secondary);",
                                "Enter the 6-digit confirmation code dispatched to your operator address."
                            }
                            div {
                                label { class: "form-label", "Verification Token" }
                                input {
                                    class: "hud-input-field",
                                    style: "letter-spacing: 0.25em; text-align: center; font-family: var(--font-mono); font-size: 16px;",
                                    placeholder: "000000",
                                    maxlength: "6",
                                    value: "{verification_code.read()}",
                                    oninput: move |e: FormEvent| verification_code.set(e.value()),
                                }
                            }
                            button {
                                class: "hud-btn primary",
                                style: "width: 100%; padding: 12px;",
                                onclick: move |_| on_success.call(()),
                                "Confirm & Continue"
                            }
                        }
                    }

                    // Cancel
                    div { style: "margin-top: 20px; text-align: center;",
                        button {
                            style: "background: none; border: none; color: var(--text-muted); font-size: 12px; cursor: pointer;",
                            onclick: move |_| on_cancel.call(()),
                            "Continue Offline as Guest"
                        }
                    }
                }
            }
        }
    }
}
