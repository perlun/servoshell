/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use cocoa::appkit::*;
use cocoa::base::*;
use cocoa::foundation::*;
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel};
use std::os::raw::c_void;
use super::window;
use super::utils;
use app::{AppEvent, AppCommand};
use state::AppState;

pub fn register() {
    let superclass = Class::get("NSResponder").unwrap();
    let mut class = ClassDecl::new("NSShellApplicationDelegate", superclass).unwrap();
    class.add_ivar::<*mut c_void>("event_queue");
    class.add_ivar::<*mut c_void>("state");

    extern fn did_finish_launching(this: &Object, _sel: Sel, _notification: id) {
        utils::get_event_queue(this).push(AppEvent::DidFinishLaunching)
    }

    extern fn did_change_screen_parameter(this: &Object, _sel: Sel, _notification: id) {
        utils::get_event_queue(this).push(AppEvent::DidChangeScreenParameters)
    }

    extern fn will_terminate(this: &Object, _sel: Sel, _notification: id) {
        utils::get_event_queue(this).push(AppEvent::WillTerminate)
    }

    extern fn validate_ui(_this: &Object, _sel: Sel, item: id) -> BOOL {
        let action: Sel = unsafe {msg_send![item, action]};
        if action == sel!(shellClearHistory:) {
            YES
        } else if action == sel!(shellToggleOptionDarkTheme:) {
            YES
        } else {
            panic!("Unexpected action to validate: {:?}", action);
        }
    }

    extern fn record_command(this: &Object, _sel: Sel, item: id) {
        let action: Sel = unsafe {msg_send![item, action]};
        let cmd = if action == sel!(shellClearHistory:) {
            AppCommand::ClearHistory
        } else if action == sel!(shellToggleOptionDarkTheme:) {
            AppCommand::ToggleOptionDarkTheme
        } else {
            panic!("Unexpected action to record: {:?}", action);
        };
        utils::get_event_queue(this).push(AppEvent::DoCommand(cmd));
    }

    unsafe {
        class.add_method(sel!(applicationDidFinishLaunching:), did_finish_launching as extern fn(&Object, Sel, id));
        class.add_method(sel!(applicationDidChangeScreenParameter:), did_change_screen_parameter as extern fn(&Object, Sel, id));
        class.add_method(sel!(applicationWillTerminate:), will_terminate as extern fn(&Object, Sel, id));

        class.add_method(sel!(validateUserInterfaceItem:), validate_ui as extern fn(&Object, Sel, id) -> BOOL);

        class.add_method(sel!(shellClearHistory:), record_command as extern fn(&Object, Sel, id));
        class.add_method(sel!(shellToggleOptionDarkTheme:), record_command as extern fn(&Object, Sel, id));
    }

    class.register();
}


pub struct App {
    nsapp: id
}

impl App {

    pub fn load() -> Result<App, &'static str> {

        let state = AppState {
            current_window_index: None,
            window_states: Vec::new(),
            dark_theme: false,
        };

        let instances = match utils::load_nib("App.nib") {
            Ok(instances) => instances,
            Err(msg) => return Err(msg),
        };

        let nsapp = instances.into_iter().find(|i| {
            utils::id_is_instance_of(*i, "NSApplication")
        });

        let nsapp: id = match nsapp {
            None => return Err("Couldn't not find NSApplication instance in nib file"),
            Some(id) => id,
        };

        unsafe {
            nsapp.setActivationPolicy_(NSApplicationActivationPolicyRegular);
            let current_app = NSRunningApplication::currentApplication(nil);
            current_app.activateWithOptions_(NSApplicationActivateIgnoringOtherApps);
        }

        // FIXME: release and set delegate to nil
        let event_queue: Vec<AppEvent> = Vec::new();
        let event_queue_ptr = Box::into_raw(Box::new(event_queue));

        let state_ptr = Box::into_raw(Box::new(state));

        unsafe {
            let delegate: id = msg_send![class("NSShellApplicationDelegate"), alloc];
            (*delegate).set_ivar("event_queue", event_queue_ptr as *mut c_void);
            (*delegate).set_ivar("state", state_ptr as *mut c_void);
            msg_send![nsapp, setDelegate:delegate];
        }

        let app = App {nsapp: nsapp};

        Ok(app)
    }

    pub fn state_changed(&self) {
        // Only the menu will be affected, and they are automatically
        // updated via validate_ui
    }

    pub fn get_events(&self) -> Vec<AppEvent> {
        let nsobject = unsafe {
            let delegate: id = msg_send![self.nsapp, delegate];
            &*delegate
        };
        utils::get_event_queue(nsobject).drain(..).collect()
    }

    // Equivalent of NSApp.run()
    pub fn run<F>(&self, mut callback: F) where F: FnMut() {

        unsafe { msg_send![self.nsapp, finishLaunching] };

        loop {
            unsafe {
                let pool = NSAutoreleasePool::new(nil);

                // Blocks until event available
                let nsevent = self.nsapp.nextEventMatchingMask_untilDate_inMode_dequeue_(
                    NSAnyEventMask.bits(),
                    NSDate::distantFuture(nil), NSDefaultRunLoopMode, YES);

                let event_type = nsevent.eventType() as u64;
                if event_type == NSApplicationDefined as u64 {
                    let event_subtype = nsevent.subtype() as i16;
                    if event_subtype == NSEventSubtype::NSApplicationActivatedEventType as i16 {
                        let nswindow: id = msg_send![nsevent, window];
                        msg_send![nswindow, eventLoopRised];
                    }
                } else {
                    msg_send![self.nsapp, sendEvent: nsevent];
                }

                // Get all pending events
                loop {
                    let nsevent = self.nsapp.nextEventMatchingMask_untilDate_inMode_dequeue_(
                        NSAnyEventMask.bits(),
                        NSDate::distantPast(nil), NSDefaultRunLoopMode, YES);
                    msg_send![self.nsapp, sendEvent: nsevent];
                    if nsevent == nil {
                        break;
                    }
                }

                msg_send![self.nsapp, updateWindows];
                msg_send![pool, release];
            }
            callback();
        }
    }

    pub fn create_window(&self) -> Result<window::Window, &'static str> {
        let (nswindow, nspopover) = match App::create_native_window() {
            Ok(w) => w,
            Err(msg) => return Err(msg),
        };

        Ok(window::Window::new(nswindow, nspopover))
    }

    fn create_native_window() -> Result<(id, id), &'static str> {
        let instances = match utils::load_nib("Window.nib") {
            Ok(instances) => instances,
            Err(msg) => return Err(msg),
        };

        let mut nspopover: Option<id> = None;
        let mut nswindow: Option<id> = None;
        for i in instances {
            let class = utils::get_classname(i);
            info!("Found class: {}", class);
            if utils::id_is_instance_of(i, "NSShellWindow") {
                nswindow = Some(i);
            }
            if utils::id_is_instance_of(i, "NSPopover") {
                nspopover = Some(i);
            }
        }

        let nswindow = match nswindow {
            None => return Err("Couldn't not find NSShellWindow instance in nib file"),
            Some(id) => id,
        };

        let nspopover = match nspopover {
            None => return Err("Couldn't not find NSPopover instance in nib file"),
            Some(id) => id,
        };

        Ok((nswindow, nspopover))
    }
}
