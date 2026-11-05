//! Native built-in tools for Windows operating system, browser automation,
//! office document production, calendar, email, travel, and shopping.

pub mod browser;
pub mod browser_runtime;
pub mod calendar;
pub mod filesystem;
pub mod mail;
pub mod memory;
pub mod office;
pub mod process;
pub mod research;
pub mod system;
pub mod task_history;
pub mod travel_shopping;
pub mod window;

pub use memory::{MemoryForgetTool, MemoryRecallTool, MemoryStoreTool};

pub use browser::{
    BrowserAttachTool, BrowserCloseTool, BrowserDownloadTool, BrowserExtractTool, BrowserFetchTool,
    BrowserGetUrlTool, BrowserLaunchTool, BrowserListPagesTool, BrowserNavigateTool,
    BrowserObserveTool, BrowserOpenTool, BrowserScreenshotTool, BrowserSwitchPageTool,
    BrowserUploadTool, BrowserWaitTool,
};
pub use browser_runtime::{
    BrowserConfig, BrowserPageInfo, BrowserRuntime, BrowserType, PageObservation,
};
pub use calendar::{CalendarCreateEventTool, CalendarDeleteEventTool, CalendarListEventsTool};
pub use filesystem::{FileDeleteTool, FileListTool, FileReadTool, FileWriteTool};
pub use mail::{
    EmailCreateDraftTool, EmailReadThreadTool, EmailSearchInboxTool, EmailSendApprovedTool,
};
pub use office::{
    OfficeDocxCreateTool, OfficeFileValidateTool, OfficePptxCreateTool, OfficeXlsxCreateTool,
};
pub use process::{ProcessInfoTool, ProcessKillTool, ProcessListTool};
pub use research::ResearchInvestigateTool;
pub use system::{SystemInfoTool, SystemScreenshotTool};
pub use task_history::{TaskArtifactsListTool, TaskHistoryGetTool};
pub use travel_shopping::{
    PaymentExecuteSandboxTestTool, ShoppingDiscoverProductsTool, TravelSearchFlightsTool,
    TravelSearchHotelsTool,
};
pub use window::{
    WindowCloseTool, WindowFocusTool, WindowListTool, WindowMaximizeTool, WindowMinimizeTool,
};
