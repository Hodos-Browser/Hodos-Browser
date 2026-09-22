#!/usr/bin/env python3
"""
Read a native macOS menu's item titles via the Accessibility API (ctypes).

⛔ Why this exists: `HUMAN_TEST_QUEUE` D9 asks whether *Inspect Element* is OFFERED in a
context menu. The menu is a native NSMenu — it is not in the DOM, and CGWindowList can
see that a menu window exists but not what is in it. pyobjc has no ApplicationServices
module on this machine, so the AX calls are bound by hand.
"""
import ctypes, ctypes.util

_AS = ctypes.CDLL(ctypes.util.find_library("ApplicationServices"))
_CF = ctypes.CDLL(ctypes.util.find_library("CoreFoundation"))

_CF.CFStringCreateWithCString.restype = ctypes.c_void_p
_CF.CFStringCreateWithCString.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_uint32]
_CF.CFStringGetCString.restype = ctypes.c_bool
_CF.CFStringGetCString.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_long, ctypes.c_uint32]
_CF.CFArrayGetCount.restype = ctypes.c_long
_CF.CFArrayGetCount.argtypes = [ctypes.c_void_p]
_CF.CFArrayGetValueAtIndex.restype = ctypes.c_void_p
_CF.CFArrayGetValueAtIndex.argtypes = [ctypes.c_void_p, ctypes.c_long]
_CF.CFGetTypeID.restype = ctypes.c_ulong
_CF.CFGetTypeID.argtypes = [ctypes.c_void_p]
_CF.CFArrayGetTypeID.restype = ctypes.c_ulong
_CF.CFStringGetTypeID.restype = ctypes.c_ulong

_AS.AXUIElementCreateApplication.restype = ctypes.c_void_p
_AS.AXUIElementCreateApplication.argtypes = [ctypes.c_int]
_AS.AXUIElementCopyAttributeValue.restype = ctypes.c_int
_AS.AXUIElementCopyAttributeValue.argtypes = [ctypes.c_void_p, ctypes.c_void_p,
                                              ctypes.POINTER(ctypes.c_void_p)]

kCFStringEncodingUTF8 = 0x08000100


def _cfstr(s):
    return _CF.CFStringCreateWithCString(None, s.encode("utf-8"), kCFStringEncodingUTF8)


def _pystr(ref):
    if not ref:
        return None
    buf = ctypes.create_string_buffer(1024)
    if _CF.CFStringGetCString(ref, buf, 1024, kCFStringEncodingUTF8):
        return buf.value.decode("utf-8", "replace")
    return None


def attr(elem, name):
    out = ctypes.c_void_p()
    if _AS.AXUIElementCopyAttributeValue(elem, _cfstr(name), ctypes.byref(out)) != 0:
        return None
    return out.value


def children(elem):
    v = attr(elem, "AXChildren")
    if not v or _CF.CFGetTypeID(v) != _CF.CFArrayGetTypeID():
        return []
    return [_CF.CFArrayGetValueAtIndex(v, i) for i in range(_CF.CFArrayGetCount(v))]


def role(elem):
    return _pystr(attr(elem, "AXRole"))


def title(elem):
    return _pystr(attr(elem, "AXTitle"))


def menu_items(pid, max_depth=6):
    """Every AXMenuItem title reachable from the application element."""
    app = _AS.AXUIElementCreateApplication(pid)
    found, seen = [], 0

    def walk(e, d):
        nonlocal seen
        if d > max_depth or seen > 4000:
            return
        seen += 1
        r = role(e)
        if r == "AXMenuItem":
            t = title(e)
            if t:
                found.append(t)
        for c in children(e):
            walk(c, d + 1)

    walk(app, 0)
    return found
