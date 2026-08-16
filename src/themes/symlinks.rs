//! Symlinks for X11 cursor names in [`super::theme::CursorTheme`].
//!
//! The first string in each list is treated as the "concrete" file that symlinks point to.
//!
//! Derived from [win2xcur-batch](https://github.com/khayalhus/win2xcur-batch/blob/main/map.json),
//! with some modifications.

use super::theme::CursorType;

pub(super) const ARROW: &[&str] = &[
    "arrow",
    "default",
    "left_ptr",
    "top_left_arrow",
    // Context menus are normally the default cursor with a menu badge.
    "context-menu",
    "08ffe1e65f80fcfdf9fff11263e74c48",
    // Drag-and-drop copy aliases use the default cursor as their closest equivalent.
    "copy",
    "dnd-copy",
    "08ffe1cb5fe6fc01f906f1c063814ccf",
    "1081e37283d90000800003c07f3ef6bf",
    "6407b0e94181790501fd1e167b474872",
    "b66166c04f8c3109214a4fbd64a50fc8",
];

pub(super) const HAND: &[&str] = &[
    "alias",
    "dnd-link",
    "hand",
    "hand2",
    "link",
    "pointer",
    "pointing_hand",
    "0876e1c15ff2fc01f906f1c363074c0f",
    "3085a0e285430894940527032f8b26df",
    "640fb0e74195791501fd1ed57b41487f",
    "9d800788f1b08800ae810202380a0822",
    "a2a266d0498c3104214a47bd64ab0fc8",
    "e29285e634086352946a0e7090d73106",
];

pub(super) const WATCH: &[&str] = &["wait", "watch", "clock", "0426c94ea35c87780ff01dc239897213"];

pub(super) const LEFT_PTR_WATCH: &[&str] = &[
    "half-busy",
    "left_ptr_watch",
    "progress",
    "00000000000000020006000e7e9ffc3f",
    "08e8e1c95fe2fc01f976f1e063a24ccd",
    "3ecb610c1bf2410f44200f48c40d3599",
    "9116a3ea924ed2162ecab71ba103b17f",
];

pub(super) const HELP: &[&str] = &[
    "dnd-ask",
    "help",
    "left_ptr_help",
    "question_arrow",
    "whats_this",
    "5c6cd98b3f3ebcb1f9c7f1c204630408",
    "d9ce0ab605698f320427677b458ad60b",
];

pub(super) const TEXT: &[&str] = &[
    "ibeam",
    "text",
    "xterm",
    // Vertical text uses a sideways I-beam, so this is only an approximation.
    "vertical-text",
    "048008013003cff3c00c801001200000",
];

pub(super) const PENCIL: &[&str] = &["draft", "pencil"];

pub(super) const CROSSHAIR: &[&str] = &[
    "cell",
    "color-picker",
    "cross_reverse",
    "cross",
    "crosshair",
    "diamond_cross",
    "plus",
    "target",
    "tcross",
];

pub(super) const FORBIDDEN: &[&str] = &[
    "circle",
    "crossed_circle",
    "dnd-no-drop",
    "forbidden",
    "not-allowed",
    "no-drop",
    "03b6e0fcb3499374a867c041f52298f0",
    "03b6e0fcb3499374a867d041f52298f0",
    "1001208387f90000800003000700f6ff",
];

pub(super) const NS_RESIZE: &[&str] = &[
    "top_side",
    "based_arrow_down",
    "based_arrow_up",
    "bottom_side",
    "down-arrow",
    "down_arrow",
    "double_arrow",
    "n-resize",
    "ns-resize",
    "row-resize",
    "s-resize",
    "sb_down_arrow",
    "sb_v_double_arrow",
    "size-ver",
    "size_ver",
    "split_v",
    "v_double_arrow",
    "00008160000006810000408080010102",
    "2870a09082c103050810ffdffffe0204",
    "c07385c7190e701020ff7ffffd08103c",
];

pub(super) const EW_RESIZE: &[&str] = &[
    "col-resize",
    "e-resize",
    "ew-resize",
    "h_double_arrow",
    "left_side",
    "left-arrow",
    "left_arrow",
    "right_side",
    "right-arrow",
    "right_arrow",
    "sb_h_double_arrow",
    "sb_left_arrow",
    "sb_right_arrow",
    "size-hor",
    "size_hor",
    "split_h",
    "w-resize",
    "14fef782d02440884392942c11205230",
    "028006030e0e7ebffc7f7070c0600140",
    "043a9f68147c53184671403ffa811cc5",
];

pub(super) const NWSE_RESIZE: &[&str] = &[
    "bd_double_arrow",
    "bottom_right_corner",
    "nw-resize",
    "nwse-resize",
    "se-resize",
    "size-fdiag",
    "size_fdiag",
    "top_left_corner",
    "lr_angle",
    "ul_angle",
    "38c5dff7c7b8962045400281044508d2",
    "c7088f0f3e6c8088236ef8e1e3e70000",
];

pub(super) const NESW_RESIZE: &[&str] = &[
    "bottom_left_corner",
    "fd_double_arrow",
    "ne-resize",
    "nesw-resize",
    "size-bdiag",
    "size_bdiag",
    "sw-resize",
    "top_right_corner",
    "ll_angle",
    "ur_angle",
    "50585d75b494802d0151028115016902",
    "fcf1c3c7cd4491d801f1e1c78f100000",
];

pub(super) const MOVE: &[&str] = &[
    "size_all",
    "all-resize",
    "all-scroll",
    "closedhand",
    "dnd-move",
    "dnd-none",
    "fleur",
    "grab",
    "hand1",
    "openhand",
    "grabbing",
    "move",
    "pointer-move",
    "208530c400c041818281048008011002",
    "5aca4d189052212118709018842178c0",
    "4498f0e0c1937ffe01fd06f973665830",
    "9081237383d90e509aa00f00170e968f",
    "fcf21c00b30f7e3f83fe0dfd12e71cff",
];

pub(super) const CENTER_PTR: &[&str] = &[
    "up_arrow",
    "up-arrow",
    "sb_up_arrow",
    "right_ptr",
    "top_right_arrow",
    "draft_large",
    "draft_small",
    "center_ptr",
];

pub(super) const fn get_symlinks(r#type: &CursorType) -> &'static [&'static str] {
    use CursorType::*;

    match r#type {
        Arrow => ARROW,
        Hand => HAND,
        Watch => WATCH,
        LeftPtrWatch => LEFT_PTR_WATCH,
        Help => HELP,
        Text => TEXT,
        Pencil => PENCIL,
        Crosshair => CROSSHAIR,
        Forbidden => FORBIDDEN,
        NsResize => NS_RESIZE,
        EwResize => EW_RESIZE,
        NwseResize => NWSE_RESIZE,
        NeswResize => NESW_RESIZE,
        Move => MOVE,
        CenterPtr => CENTER_PTR,
    }
}
