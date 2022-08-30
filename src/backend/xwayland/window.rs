use crate::backend::xwayland::Atoms;
use std::fmt;
use crate::backend::xwayland::error::XWaylandError;

/// WinType provides an easy way to identify the different window types
#[allow(dead_code)]
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum WinType {
    Combo,
    Desktop,
    Dialog,
    Dnd,
    Dock,
    DropDownMenu,
    Menu,
    Normal,
    Notification,
    PopupMenu,
    Splash,
    Toolbar,
    ToolTip,
    Utility,
    Invalid, // made up value to track missing
}

// Convert from u32 to Type
impl WinType {
    pub fn from(atoms: &Atoms, val: u32) -> Result<WinType, XWaylandError> {
        if val == atoms._NET_WM_WINDOW_TYPE_COMBO {
            Ok(WinType::Combo)
        } else if val == atoms._NET_WM_WINDOW_TYPE_DESKTOP {
            Ok(WinType::Desktop)
        } else if val == atoms._NET_WM_WINDOW_TYPE_DIALOG {
            Ok(WinType::Dialog)
        } else if val == atoms._NET_WM_WINDOW_TYPE_DND {
            Ok(WinType::Dnd)
        } else if val == atoms._NET_WM_WINDOW_TYPE_DOCK {
            Ok(WinType::Dock)
        } else if val == atoms._NET_WM_WINDOW_TYPE_DROPDOWN_MENU {
            Ok(WinType::DropDownMenu)
        } else if val == atoms._NET_WM_WINDOW_TYPE_MENU {
            Ok(WinType::Menu)
        } else if val == atoms._NET_WM_WINDOW_TYPE_NORMAL {
            Ok(WinType::Normal)
        } else if val == atoms._NET_WM_WINDOW_TYPE_NOTIFICATION {
            Ok(WinType::Notification)
        } else if val == atoms._NET_WM_WINDOW_TYPE_POPUP_MENU {
            Ok(WinType::PopupMenu)
        } else if val == atoms._NET_WM_WINDOW_TYPE_SPLASH {
            Ok(WinType::Splash)
        } else if val == atoms._NET_WM_WINDOW_TYPE_TOOLBAR {
            Ok(WinType::Toolbar)
        } else if val == atoms._NET_WM_WINDOW_TYPE_TOOLTIP {
            Ok(WinType::ToolTip)
        } else if val == atoms._NET_WM_WINDOW_TYPE_UTILITY {
            Ok(WinType::Utility)
        } else {
            Err(XWaylandError::UnknownAtom(val))
        }
    }
}

// Implement format! support
impl fmt::Display for WinType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WinType::Invalid => write!(f, ""),
            _ => write!(f, "{}", format!("{:?}", self).to_lowercase()),
        }
    }
}
