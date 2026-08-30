//! macOS Dock 우클릭 메뉴: New Window
#![cfg(target_os = "macos")]

use std::cell::OnceCell;
use std::sync::OnceLock;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, sel, MainThreadOnly};
use objc2_app_kit::{NSApplication, NSMenu, NSMenuItem};
use objc2_foundation::{ns_string, MainThreadMarker, NSObject, NSObjectProtocol, NSString};
use tauri::AppHandle;

use crate::windows::open_window;

static APP: OnceLock<AppHandle> = OnceLock::new();

thread_local! {
    static KEEP: OnceCell<(Retained<NSMenu>, Retained<DockTarget>)> = const { OnceCell::new() };
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "TypeboardDockTarget"]
    #[thread_kind = MainThreadOnly]
    #[ivars = ()]
    struct DockTarget;

    unsafe impl NSObjectProtocol for DockTarget {}

    impl DockTarget {
        #[unsafe(method(openNewWindow:))]
        fn open_new_window(&self, _sender: Option<&AnyObject>) {
            if let Some(app) = APP.get() {
                let _ = open_window(app);
            }
        }
    }
);

impl DockTarget {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc().set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

pub fn install(app: &AppHandle) {
    let _ = APP.set(app.clone());
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };

    let target = DockTarget::new(mtm);
    let menu = NSMenu::new(mtm);
    let title = NSString::from_str("New Window");
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &title,
            Some(sel!(openNewWindow:)),
            ns_string!(""),
        )
    };
    unsafe {
        item.setTarget(Some(AsRef::<AnyObject>::as_ref(&*target)));
        item.setEnabled(true);
        menu.addItem(&item);
        menu.setAutoenablesItems(false);
    }

    let ns_app = NSApplication::sharedApplication(mtm);
    let _: () = unsafe { msg_send![&ns_app, setValue: &*menu, forKey: ns_string!("dockMenu")] };

    KEEP.with(|cell| {
        let _ = cell.set((menu, target));
    });
}
