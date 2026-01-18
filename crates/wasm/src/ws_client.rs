//! WebSocket client for WASM

use overachiever_core::{ClientMessage, ServerMessage};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{MessageEvent, WebSocket, ErrorEvent, CloseEvent};

#[derive(Clone, PartialEq)]
#[allow(dead_code)]
pub enum WsState {
    Connecting,
    Open,
    Closing,
    Closed,
    Error(String),
}

pub struct WsClient {
    ws: WebSocket,
    messages: Rc<RefCell<Vec<ServerMessage>>>,
    state: Rc<RefCell<WsState>>,
}

impl WsClient {
    pub fn new(url: &str) -> Result<Self, String> {
        let ws = WebSocket::new(url).map_err(|e| format!("Failed to create WebSocket: {:?}", e))?;
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);
        
        let messages: Rc<RefCell<Vec<ServerMessage>>> = Rc::new(RefCell::new(Vec::new()));
        let state: Rc<RefCell<WsState>> = Rc::new(RefCell::new(WsState::Connecting));
        
        // Set up onmessage handler
        {
            let messages = messages.clone();
            let onmessage = Closure::<dyn FnMut(_)>::new(move |e: MessageEvent| {
                if let Ok(text) = e.data().dyn_into::<js_sys::JsString>() {
                    let text: String = text.into();
                    if let Ok(msg) = serde_json::from_str::<ServerMessage>(&text) {
                        messages.borrow_mut().push(msg);
                    }
                }
            });
            ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
            onmessage.forget();
        }
        
        // Set up onopen handler
        {
            let state = state.clone();
            let onopen = Closure::<dyn FnMut()>::new(move || {
                *state.borrow_mut() = WsState::Open;
            });
            ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
            onopen.forget();
        }
        
        // Set up onerror handler
        {
            let state = state.clone();
            let onerror = Closure::<dyn FnMut(_)>::new(move |_e: ErrorEvent| {
                *state.borrow_mut() = WsState::Error("WebSocket error".to_string());
            });
            ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
            onerror.forget();
        }
        
        // Set up onclose handler
        {
            let state = state.clone();
            let onclose = Closure::<dyn FnMut(_)>::new(move |_e: CloseEvent| {
                *state.borrow_mut() = WsState::Closed;
            });
            ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
            onclose.forget();
        }
        
        Ok(Self { ws, messages, state })
    }
    
    pub fn state(&self) -> WsState {
        self.state.borrow().clone()
    }
    
    pub fn poll_messages(&self) -> Vec<ServerMessage> {
        self.messages.borrow_mut().drain(..).collect()
    }
    
    fn send(&self, msg: &ClientMessage) {
        if let Ok(json) = serde_json::to_string(msg) {
            let _ = self.ws.send_with_str(&json);
        }
    }
    
    pub fn authenticate(&self, token: &str) {
        self.send(&ClientMessage::Authenticate { token: token.to_string() });
    }
    
    pub fn fetch_games(&self) {
        self.send(&ClientMessage::FetchGames);
    }
    
    pub fn fetch_achievements(&self, appid: u64) {
        self.send(&ClientMessage::FetchAchievements { appid });
    }
    
    pub fn sync_from_steam(&self) {
        self.send(&ClientMessage::SyncFromSteam);
    }
    
    pub fn full_scan(&self, force: bool) {
        self.send(&ClientMessage::FullScan { force });
    }
    
    pub fn refresh_single_game(&self, appid: u64) {
        self.send(&ClientMessage::RefreshSingleGame { appid });
    }
    
    pub fn fetch_history(&self) {
        self.send(&ClientMessage::FetchHistory);
    }
    
    /// View another user's library by short_id (no authentication required)
    pub fn view_guest_library(&self, short_id: &str) {
        self.send(&ClientMessage::ViewGuestLibrary { short_id: short_id.to_string() });
    }
    
    /// Fetch achievements for a game when viewing as guest
    pub fn fetch_guest_achievements(&self, short_id: &str, appid: u64) {
        self.send(&ClientMessage::FetchGuestAchievements { 
            short_id: short_id.to_string(), 
            appid 
        });
    }
    
    /// Fetch history when viewing as guest
    pub fn fetch_guest_history(&self, short_id: &str) {
        self.send(&ClientMessage::FetchGuestHistory { short_id: short_id.to_string() });
    }
    
    pub fn report_ttb(&self, appid: u64, main_seconds: Option<i32>, extra_seconds: Option<i32>, completionist_seconds: Option<i32>) {
        self.send(&ClientMessage::ReportTtb { 
            appid, 
            main_seconds, 
            extra_seconds, 
            completionist_seconds 
        });
    }
    
    #[allow(dead_code)]
    pub fn submit_rating(&self, appid: u64, rating: u8, comment: Option<String>) {
        self.send(&ClientMessage::SubmitRating { appid, rating, comment });
    }
    
    #[allow(dead_code)]
    pub fn get_community_ratings(&self, appid: u64) {
        self.send(&ClientMessage::GetCommunityRatings { appid });
    }
}

impl Drop for WsClient {
    fn drop(&mut self) {
        let _ = self.ws.close();
    }
}
