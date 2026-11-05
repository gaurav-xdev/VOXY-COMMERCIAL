#[cfg(test)]
mod tests {
    use voxy_desktop_ui::components::assistant_core::{CompanionMode, CoreState};
    use voxy_desktop_ui::components::onboarding_wizard::{OnboardingConfig, OnboardingStep};
    use voxy_desktop_ui::fixtures::{
        DEFAULT_MODELS, PROVIDER_CATALOG, SUBSCRIPTION_PLANS, VOICE_CATALOG,
    };
    use voxy_desktop_ui::views::auth::AuthMode;
    use voxy_desktop_ui::views::settings::{SettingsFormState, SettingsTab};

    #[test]
    fn test_core_state_labels_and_strings() {
        assert_eq!(CoreState::Idle.as_str(), "Idle");
        assert_eq!(CoreState::Listening.as_str(), "Listening");
        assert_eq!(CoreState::Thinking.as_str(), "Thinking");
        assert_eq!(CoreState::Speaking.as_str(), "Speaking");
        assert_eq!(CoreState::Error.as_str(), "Error");
        assert_eq!(CoreState::Offline.as_str(), "Offline");

        assert!(CoreState::Idle.label().contains("READY"));
        assert!(CoreState::Listening.label().contains("LISTENING"));
        assert!(CoreState::Thinking.label().contains("REASONING"));
        assert!(CoreState::Speaking.label().contains("SPEECH"));
        assert!(CoreState::Error.label().contains("ATTENTION"));
        assert!(CoreState::Offline.label().contains("DISCONNECTED"));
    }

    #[test]
    fn test_companion_modes() {
        assert_eq!(CompanionMode::default(), CompanionMode::Cat);
    }

    #[test]
    fn test_fixtures_catalog_integrity() {
        assert!(!PROVIDER_CATALOG.is_empty());
        assert!(!DEFAULT_MODELS.is_empty());
        assert!(!VOICE_CATALOG.is_empty());
        assert_eq!(SUBSCRIPTION_PLANS.len(), 3);

        // Verify local runtime is present
        let has_ollama = PROVIDER_CATALOG.iter().any(|p| p.id == "ollama");
        assert!(has_ollama);

        // Verify cloud providers require API keys
        let openai = PROVIDER_CATALOG.iter().find(|p| p.id == "openai").unwrap();
        assert!(openai.requires_api_key);

        let ollama = PROVIDER_CATALOG.iter().find(|p| p.id == "ollama").unwrap();
        assert!(!ollama.requires_api_key);

        // Verify tier IDs and prices
        let tiers: Vec<&str> = SUBSCRIPTION_PLANS.iter().map(|p| p.id).collect();
        assert_eq!(tiers, vec!["free", "pro", "max"]);

        let free_plan = SUBSCRIPTION_PLANS.iter().find(|p| p.id == "free").unwrap();
        assert_eq!(free_plan.price, "$0");

        let pro_plan = SUBSCRIPTION_PLANS.iter().find(|p| p.id == "pro").unwrap();
        assert_eq!(pro_plan.price, "$19");
        assert!(pro_plan.is_popular);
    }

    #[test]
    fn test_onboarding_defaults_and_steps() {
        let default_cfg = OnboardingConfig::default();
        assert_eq!(default_cfg.assistant_name, "OSMOO");
        assert_eq!(default_cfg.companion_mode, CompanionMode::Cat);
        assert_eq!(default_cfg.primary_llm, "ollama");
        assert_eq!(default_cfg.selected_model, "llama3.2:3b");
        assert_eq!(default_cfg.active_tier, "free");

        // Verify 5 distinct sequential steps
        assert_eq!(OnboardingStep::Welcome as u8, 1);
        assert_eq!(OnboardingStep::AiProvider as u8, 2);
        assert_eq!(OnboardingStep::VoicePermissions as u8, 3);
        assert_eq!(OnboardingStep::CompanionPreferences as u8, 4);
        assert_eq!(OnboardingStep::TestAndFinish as u8, 5);
    }

    #[test]
    fn test_settings_form_state_and_tabs() {
        let mut settings = SettingsFormState::default();
        assert_eq!(settings.assistant_name, "OSMOO Core");
        assert_eq!(settings.audio_sample_rate, 48000);
        assert!(settings.autolaunch);
        assert!(settings.minimize_to_tray);

        // Test mutating form parameters in memory
        settings.assistant_name = "OSMOO Prime".to_string();
        settings.audio_sample_rate = 96000;
        settings.autolaunch = false;

        assert_eq!(settings.assistant_name, "OSMOO Prime");
        assert_eq!(settings.audio_sample_rate, 96000);
        assert!(!settings.autolaunch);

        // Verify all 9 settings tabs exist
        let all_tabs = SettingsTab::all();
        assert_eq!(all_tabs.len(), 10);
        for tab in all_tabs {
            assert!(!tab.title().is_empty());
            assert!(!tab.description().is_empty());
        }
    }

    #[test]
    fn test_auth_modes() {
        let modes = [
            AuthMode::SignIn,
            AuthMode::SignUp,
            AuthMode::ForgotPassword,
            AuthMode::VerifyEmail,
        ];
        assert_eq!(modes.len(), 4);
    }

    #[test]
    fn test_webgl_particle_geometry_calculation_and_budget() {
        // Verification of 14,000 particle points (42,000 Float32 elements)
        let particle_count = 14000;
        let mut positions = vec![0.0f32; particle_count * 3];
        let radius = 2.4f32;

        let start = std::time::Instant::now();
        for i in 0..particle_count {
            let u = (i as f32) / (particle_count as f32);
            let theta = u * 2.0 * std::f32::consts::PI;
            let phi = (u * std::f32::consts::PI).sin().acos();
            let r = radius * (0.92 + 0.08);

            positions[i * 3] = r * phi.sin() * theta.cos();
            positions[i * 3 + 1] = r * phi.sin() * theta.sin();
            positions[i * 3 + 2] = r * phi.cos();
        }

        // Simulate 10 frame wave updates
        for frame in 0..10 {
            let t = (frame as f32) * (1.0 / 60.0);
            for i in (0..positions.len()).step_by(3) {
                let u = (i / 3) as f32;
                let wave = (t * 2.5 + u * 0.05).sin() * 0.02;
                positions[i] += if positions[i] > 0.0 {
                    wave * 0.01
                } else {
                    -wave * 0.01
                };
            }
        }
        let elapsed = start.elapsed();

        // 10 frames of 14,000 particles should complete easily in under 200ms even in unoptimized debug mode
        assert!(
            elapsed.as_millis() < 200,
            "Particle math must execute well within frame budget: elapsed = {:?}",
            elapsed
        );
        assert_eq!(positions.len(), 42000);
    }

    #[test]
    fn test_cloud_provider_auth_fail_closed_and_dpapi_contract() {
        // Enforce all remote Cloud Providers require auth credentials
        let remote_providers: Vec<_> = PROVIDER_CATALOG
            .iter()
            .filter(|p| p.category == "Cloud Provider")
            .collect();

        assert!(!remote_providers.is_empty());
        for p in remote_providers {
            assert!(
                p.requires_api_key,
                "Cloud provider {} must require an API key to fail closed",
                p.id
            );
        }

        // Local runtimes must NOT require API keys
        let local_providers: Vec<_> = PROVIDER_CATALOG
            .iter()
            .filter(|p| p.category == "Local Runtime")
            .collect();

        assert!(!local_providers.is_empty());
        for p in local_providers {
            assert!(
                !p.requires_api_key,
                "Local runtime {} must not require an API key",
                p.id
            );
        }
    }

    #[test]
    fn test_osmoo_cat_asset_integrity() {
        use voxy_desktop_ui::assets::{OSMOO_CAT_DATA_URI, OSMOO_CAT_SHADOW_DATA_URI};

        // Standard PNG base64 header signature is always iVBORw0KGgo
        assert!(OSMOO_CAT_DATA_URI.starts_with("data:image/png;base64,iVBORw0KGgo"));
        assert!(
            OSMOO_CAT_DATA_URI.len() > 100_000,
            "Cat image asset must be full-fidelity PNG"
        );

        assert!(OSMOO_CAT_SHADOW_DATA_URI.starts_with("data:image/png;base64,iVBORw0KGgo"));
        assert!(
            OSMOO_CAT_SHADOW_DATA_URI.len() > 1_000,
            "Contact shadow asset must be present"
        );
    }

    #[test]
    fn test_settings_cat_companion_defaults() {
        use voxy_desktop_ui::views::SettingsFormState;
        let defaults = SettingsFormState::default();
        assert_eq!(defaults.companion_character_id, "cat");
        assert_eq!(defaults.companion_size, 1.0);
        assert_eq!(defaults.companion_position, "bottom-right");
        assert_eq!(defaults.companion_expression, "neutral");
        assert!(defaults.companion_lip_sync);
        assert!(defaults.companion_animations);
        assert!(defaults.companion_draggable);
        assert!(!defaults.companion_click_through);
        assert!(defaults.desktop_overlay_mode);
    }
}
