#!/usr/bin/env python3

from __future__ import annotations

import argparse
import sys
import time
from dataclasses import dataclass

try:
    from Xlib import X, XK, display
    from Xlib.ext import xtest
    from Xlib.protocol import event
except ImportError as exc:
    raise SystemExit(
        "python-xlib is required. Install it in a temp venv or user environment before running this helper."
    ) from exc


@dataclass
class WindowInfo:
    window_id: int
    title: str
    class_name: str


MODIFIER_KEY_NAMES = {
    "ctrl": "Control_L",
    "control": "Control_L",
    "shift": "Shift_L",
    "alt": "Alt_L",
    "meta": "Meta_L",
    "super": "Super_L",
}

SPECIAL_KEY_NAMES = {
    "enter": "Return",
    "return": "Return",
    "space": "space",
    "tab": "Tab",
    "esc": "Escape",
    "escape": "Escape",
    "backspace": "BackSpace",
}


def connect() -> display.Display:
    return display.Display()


def get_window_title(window) -> str:
    for getter in ("get_full_text_property", "get_wm_name"):
        try:
            if getter == "get_full_text_property":
                atom = window.display.get_atom("_NET_WM_NAME")
                value = getattr(window, getter)(atom)
            else:
                value = getattr(window, getter)()
            if value:
                return value.decode() if isinstance(value, bytes) else str(value)
        except Exception:
            continue
    return ""


def get_window_class(window) -> str:
    try:
        value = window.get_wm_class()
    except Exception:
        return ""
    if not value:
        return ""
    return " ".join(part for part in value if part)


def walk_windows(root) -> list[WindowInfo]:
    results: list[WindowInfo] = []

    def visit(window) -> None:
        title = get_window_title(window).strip()
        class_name = get_window_class(window).strip()
        if title or class_name:
            results.append(WindowInfo(window.id, title, class_name))
        try:
            children = window.query_tree().children
        except Exception:
            return
        for child in children:
            visit(child)

    visit(root)
    return results


def find_window(disp: display.Display, title: str | None, class_name: str | None) -> WindowInfo:
    root = disp.screen().root
    matches = []
    title = title.lower() if title else None
    class_name = class_name.lower() if class_name else None

    for info in walk_windows(root):
        title_ok = title is None or title in info.title.lower()
        class_ok = class_name is None or class_name in info.class_name.lower()
        if title_ok and class_ok:
            matches.append(info)

    if not matches:
        criteria = []
        if title:
            criteria.append(f"title containing {title!r}")
        if class_name:
            criteria.append(f"class containing {class_name!r}")
        raise SystemExit(f"No X11 window matched {' and '.join(criteria) or 'the requested criteria'}.")

    return matches[0]


def activate_window(disp: display.Display, info: WindowInfo) -> None:
    root = disp.screen().root
    window = disp.create_resource_object("window", info.window_id)
    active_atom = disp.intern_atom("_NET_ACTIVE_WINDOW")

    client_message = event.ClientMessage(
        window=window,
        client_type=active_atom,
        data=(32, [1, X.CurrentTime, 0, 0, 0]),
    )
    root.send_event(
        client_message,
        event_mask=X.SubstructureRedirectMask | X.SubstructureNotifyMask,
    )
    try:
        window.set_input_focus(X.RevertToParent, X.CurrentTime)
    except Exception:
        pass
    disp.sync()


def keysym_name(name: str) -> str:
    lowered = name.strip().lower()
    if lowered in MODIFIER_KEY_NAMES:
        return MODIFIER_KEY_NAMES[lowered]
    if lowered in SPECIAL_KEY_NAMES:
        return SPECIAL_KEY_NAMES[lowered]
    if len(name) == 1:
        return name
    return name


def keycode_for_name(disp: display.Display, name: str) -> int:
    keysym = XK.string_to_keysym(keysym_name(name))
    if keysym == 0:
        raise SystemExit(f"Unknown key: {name}")
    keycode = disp.keysym_to_keycode(keysym)
    if keycode == 0:
        raise SystemExit(f"Unable to resolve X11 keycode for: {name}")
    return keycode


def send_combo(disp: display.Display, combo: str, pause_ms: int) -> None:
    parts = [part.strip() for part in combo.split("+") if part.strip()]
    if not parts:
        raise SystemExit("Key combo cannot be empty.")

    modifier_parts = parts[:-1]
    main_key = parts[-1]

    modifier_codes = [keycode_for_name(disp, part) for part in modifier_parts]
    main_code = keycode_for_name(disp, main_key)

    for code in modifier_codes:
        xtest.fake_input(disp, X.KeyPress, code)
    xtest.fake_input(disp, X.KeyPress, main_code)
    xtest.fake_input(disp, X.KeyRelease, main_code)
    for code in reversed(modifier_codes):
        xtest.fake_input(disp, X.KeyRelease, code)
    disp.sync()
    time.sleep(max(pause_ms, 0) / 1000)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Minimal X11 automation helper for desktop verification.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    list_windows = subparsers.add_parser("list-windows", help="List visible X11 windows.")
    list_windows.add_argument("--contains", help="Optional title/class substring filter.")

    focus = subparsers.add_parser("focus", help="Focus the first X11 window that matches.")
    focus.add_argument("--title", help="Window title substring.")
    focus.add_argument("--class-name", help="Window class substring.")
    focus.add_argument("--pause-ms", type=int, default=350)

    send_keys = subparsers.add_parser("send-keys", help="Send one or more key combos through XTEST.")
    send_keys.add_argument("--keys", action="append", required=True, help="Key combo such as ctrl+shift+Return.")
    send_keys.add_argument("--pause-ms", type=int, default=250)

    return parser.parse_args()


def main() -> None:
    args = parse_args()
    disp = connect()

    if args.command == "list-windows":
        needle = args.contains.lower() if args.contains else None
        for info in walk_windows(disp.screen().root):
            haystack = f"{info.title} {info.class_name}".lower()
            if needle and needle not in haystack:
                continue
            print(f"0x{info.window_id:x}\t{info.title}\t{info.class_name}")
        return

    if args.command == "focus":
        info = find_window(disp, args.title, args.class_name)
        activate_window(disp, info)
        time.sleep(max(args.pause_ms, 0) / 1000)
        print(f"focused 0x{info.window_id:x} {info.title or info.class_name}")
        return

    if args.command == "send-keys":
        for combo in args.keys:
            send_combo(disp, combo, args.pause_ms)
        print("sent")
        return

    raise SystemExit(f"Unhandled command: {args.command}")


if __name__ == "__main__":
    main()
