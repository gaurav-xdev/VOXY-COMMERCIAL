#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Route {
    Core,
    Chat,
    Settings,
    Memory,
    Plugins,
    Downloads,
    Notifications,
    Account,
    Subscription,
    Login,
    Health,
    Orb,
}

#[allow(dead_code)]
impl Route {
    pub fn label(&self) -> &'static str {
        match self {
            Route::Core => "OSMOO Core",
            Route::Chat => "Chat",
            Route::Settings => "Settings",
            Route::Memory => "Memory",
            Route::Plugins => "Plugins",
            Route::Downloads => "Downloads",
            Route::Notifications => "Notifications",
            Route::Account => "Account",
            Route::Subscription => "Subscription",
            Route::Login => "Login",
            Route::Health => "Health",
            Route::Orb => "Voice Orb",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Route::Core => "\u{25C9}",
            Route::Chat => "\u{1F4AC}",
            Route::Settings => "\u{2699}",
            Route::Memory => "\u{1F4E6}",
            Route::Plugins => "\u{1F50C}",
            Route::Downloads => "\u{2B07}",
            Route::Notifications => "\u{1F514}",
            Route::Account => "\u{1F464}",
            Route::Subscription => "\u{2B50}",
            Route::Login => "\u{1F511}",
            Route::Health => "\u{1F4CA}",
            Route::Orb => "\u{1F300}",
        }
    }
}
