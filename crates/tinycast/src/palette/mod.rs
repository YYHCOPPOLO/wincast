pub(crate) mod d2d;
mod edit;
mod hwnd;
pub(crate) mod menu;

pub use hwnd::PaletteWindow;

pub mod physical {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Rect {
        pub x: i32,
        pub y: i32,
        pub w: i32,
        pub h: i32,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Point {
        pub x: i32,
        pub y: i32,
    }

    impl Rect {
        pub fn from_win32(r: windows::Win32::Foundation::RECT) -> Self {
            Self {
                x: r.left,
                y: r.top,
                w: r.right - r.left,
                h: r.bottom - r.top,
            }
        }
    }
}
