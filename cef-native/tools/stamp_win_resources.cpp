// stamp_win_resources — brand a copied CEF bootstrap.exe with Hodos resources.
//
// WHY THIS EXISTS
// ---------------
// Under the CEF >= 150 bootstrap model (upstream #3928) our application is a DLL and the
// executable is CEF's own bootstrap.exe, copied and renamed to HodosBrowser.exe by a CMake
// POST_BUILD step. It is byte-identical to CEF's binary, which means it also carries CEF's
// icon and CEF's version resources — that is why Explorer, the taskbar button and the
// pin-to-taskbar entry all showed the wrong logo. Our hodos.ico only ever landed in
// HodosBrowser.dll, which the shell never displays.
//
// Patching CEF's bootstrap.rc through the P3 patch toolchain was considered and rejected:
// it would need a full Chromium rebuild per icon change and would weld our branding into
// CEF. Instead we post-process the copied exe with the Win32 resource-update API.
//
// USAGE
//   stamp_win_resources <target.exe> <icon.ico> <version>
//
//   <version> is the CMake APP_VERSION, e.g. "0.4.1" or "0.4.1-beta.29". The numeric
//   prefix feeds VS_FIXEDFILEINFO; the full string is kept verbatim in the FileVersion
//   and ProductVersion strings so a beta suffix survives.
//
// SAFETY NOTES (both learned the hard way — do not "simplify" these away)
//   * BeginUpdateResource is called with bDeleteExistingResources = FALSE. Passing TRUE
//     would also wipe RT_MANIFEST, and under the bootstrap model the process manifest —
//     including the Win10/11 supportedOS GUIDs — comes from bootstrap.exe. Destroying it
//     would silently change OS version reporting and DPI behaviour.
//   * CEF's own icons are enumerated and deleted rather than assumed. Explorer picks the
//     RT_GROUP_ICON with the LOWEST id, so simply adding ours alongside CEF's would work
//     today (1 < 32512) and break the day CEF renumbers. We remove what is there.

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif

#include <windows.h>

#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

namespace {

// ---------------------------------------------------------------------------
// .ico file layout. These are the on-disk structures; the in-resource group
// structure differs (see GrpIconDirEntry) and that difference is the single
// most common source of "the icon is there but Windows won't show it" bugs.
// ---------------------------------------------------------------------------
#pragma pack(push, 1)
struct IconDir {
  WORD reserved;  // 0
  WORD type;      // 1 = icon
  WORD count;
};
struct IconDirEntry {
  BYTE width;
  BYTE height;
  BYTE color_count;
  BYTE reserved;
  WORD planes;
  WORD bit_count;
  DWORD bytes_in_res;
  DWORD image_offset;  // file offset — replaced by a resource id in the group
};
struct GrpIconDirEntry {
  BYTE width;
  BYTE height;
  BYTE color_count;
  BYTE reserved;
  WORD planes;
  WORD bit_count;
  DWORD bytes_in_res;
  WORD id;  // RT_ICON resource id, NOT a file offset
};
#pragma pack(pop)

const WORD kLangEnUs = MAKELANGID(LANG_ENGLISH, SUBLANG_ENGLISH_US);

// Branding. Kept in lockstep with installer/hodos-browser.iss (AppName / AppPublisher).
const wchar_t* kCompanyName = L"Hodos";
const wchar_t* kProductName = L"Hodos Browser";
const wchar_t* kFileDescription = L"Hodos Browser";
const wchar_t* kOriginalFilename = L"HodosBrowser.exe";
const wchar_t* kLegalCopyright = L"Copyright (C) 2026 Hodos";

void Fail(const char* what) {
  std::fprintf(stderr, "stamp_win_resources: %s (GetLastError=%lu)\n", what, GetLastError());
}

bool ReadWholeFile(const std::wstring& path, std::vector<BYTE>* out) {
  HANDLE h = CreateFileW(path.c_str(), GENERIC_READ, FILE_SHARE_READ, nullptr, OPEN_EXISTING,
                         FILE_ATTRIBUTE_NORMAL, nullptr);
  if (h == INVALID_HANDLE_VALUE) return false;
  LARGE_INTEGER size{};
  if (!GetFileSizeEx(h, &size) || size.QuadPart <= 0 || size.QuadPart > (64 << 20)) {
    CloseHandle(h);
    return false;
  }
  out->resize(static_cast<size_t>(size.QuadPart));
  DWORD read = 0;
  const bool ok = ReadFile(h, out->data(), static_cast<DWORD>(out->size()), &read, nullptr) &&
                  read == out->size();
  CloseHandle(h);
  return ok;
}

// ---------------------------------------------------------------------------
// Enumerate existing (name, language) pairs so they can be deleted explicitly.
// Deleting requires the exact language of the existing resource, so both levels
// have to be walked — assuming en-US would leave neutral-language icons behind.
// ---------------------------------------------------------------------------
struct ResourceKey {
  std::wstring name;   // used when is_ordinal is false
  WORD ordinal = 0;    // used when is_ordinal is true
  bool is_ordinal = false;
  WORD lang = 0;

  LPCWSTR NamePtr() const {
    return is_ordinal ? MAKEINTRESOURCEW(ordinal) : name.c_str();
  }
};

struct EnumCtx {
  HMODULE module = nullptr;
  LPCWSTR type = nullptr;
  std::vector<ResourceKey>* out = nullptr;
  ResourceKey current;
};

BOOL CALLBACK EnumLangProc(HMODULE, LPCWSTR, LPCWSTR, WORD lang, LONG_PTR param) {
  auto* ctx = reinterpret_cast<EnumCtx*>(param);
  ResourceKey key = ctx->current;
  key.lang = lang;
  ctx->out->push_back(key);
  return TRUE;
}

BOOL CALLBACK EnumNameProc(HMODULE module, LPCWSTR type, LPWSTR name, LONG_PTR param) {
  auto* ctx = reinterpret_cast<EnumCtx*>(param);
  ctx->current = ResourceKey{};
  if (IS_INTRESOURCE(name)) {
    ctx->current.is_ordinal = true;
    ctx->current.ordinal = static_cast<WORD>(reinterpret_cast<ULONG_PTR>(name));
  } else {
    ctx->current.name = name;
  }
  EnumResourceLanguagesW(module, type, name, EnumLangProc, param);
  return TRUE;
}

std::vector<ResourceKey> ListResources(const std::wstring& exe, LPCWSTR type) {
  std::vector<ResourceKey> keys;
  HMODULE mod = LoadLibraryExW(exe.c_str(), nullptr, LOAD_LIBRARY_AS_DATAFILE);
  if (!mod) return keys;
  EnumCtx ctx;
  ctx.module = mod;
  ctx.type = type;
  ctx.out = &keys;
  EnumResourceNamesW(mod, type, EnumNameProc, reinterpret_cast<LONG_PTR>(&ctx));
  FreeLibrary(mod);
  return keys;
}

// ---------------------------------------------------------------------------
// VS_VERSIONINFO writer.
//
// Every node is: WORD wLength, WORD wValueLength, WORD wType, WCHAR szKey[],
// <pad to DWORD>, value, <pad to DWORD>, children. wValueLength is a BYTE count
// for binary nodes but a WCHAR count (including the null) for text nodes — that
// asymmetry is in the spec and getting it wrong yields a version block that
// Explorer's property sheet silently refuses to display.
// ---------------------------------------------------------------------------
class VersionWriter {
 public:
  size_t Begin(const wchar_t* key, WORD value_len, WORD type) {
    Align();
    const size_t pos = buf_.size();
    U16(0);  // wLength placeholder, patched in End()
    U16(value_len);
    U16(type);
    WStr(key);
    Align();
    return pos;
  }

  void End(size_t pos) {
    const WORD len = static_cast<WORD>(buf_.size() - pos);
    std::memcpy(&buf_[pos], &len, sizeof(WORD));
  }

  void AddString(const wchar_t* key, const std::wstring& value) {
    // wValueLength counts WCHARs including the terminator for text nodes.
    const WORD chars = static_cast<WORD>(value.size() + 1);
    const size_t pos = Begin(key, chars, 1 /*text*/);
    WStr(value.c_str());
    End(pos);
  }

  void Align() {
    while (buf_.size() % 4 != 0) buf_.push_back(0);
  }
  void U16(WORD v) { Append(&v, sizeof(v)); }
  void U32(DWORD v) { Append(&v, sizeof(v)); }
  void WStr(const wchar_t* s) {
    Append(s, (std::wcslen(s) + 1) * sizeof(wchar_t));
  }
  void Append(const void* p, size_t n) {
    const BYTE* b = static_cast<const BYTE*>(p);
    buf_.insert(buf_.end(), b, b + n);
  }

  std::vector<BYTE>& buf() { return buf_; }

 private:
  std::vector<BYTE> buf_;
};

// Parse a leading "MAJOR.MINOR.PATCH" out of e.g. "0.4.1-beta.29".
void ParseVersion(const std::wstring& v, WORD parts[4]) {
  parts[0] = parts[1] = parts[2] = parts[3] = 0;
  int idx = 0;
  unsigned cur = 0;
  bool any = false;
  for (wchar_t c : v) {
    if (c >= L'0' && c <= L'9') {
      cur = cur * 10 + static_cast<unsigned>(c - L'0');
      if (cur > 65535) cur = 65535;
      any = true;
    } else if (c == L'.' && any) {
      parts[idx++] = static_cast<WORD>(cur);
      cur = 0;
      any = false;
      if (idx == 4) return;
    } else {
      break;  // stop at the first non-numeric segment, e.g. "-beta"
    }
  }
  if (any && idx < 4) parts[idx] = static_cast<WORD>(cur);
}

std::vector<BYTE> BuildVersionResource(const std::wstring& version) {
  WORD p[4];
  ParseVersion(version, p);

  VS_FIXEDFILEINFO ffi{};
  ffi.dwSignature = 0xFEEF04BD;
  ffi.dwStrucVersion = 0x00010000;
  ffi.dwFileVersionMS = (static_cast<DWORD>(p[0]) << 16) | p[1];
  ffi.dwFileVersionLS = (static_cast<DWORD>(p[2]) << 16) | p[3];
  ffi.dwProductVersionMS = ffi.dwFileVersionMS;
  ffi.dwProductVersionLS = ffi.dwFileVersionLS;
  ffi.dwFileFlagsMask = VS_FFI_FILEFLAGSMASK;
  ffi.dwFileFlags = 0;
  ffi.dwFileOS = VOS__WINDOWS32;
  ffi.dwFileType = VFT_APP;
  ffi.dwFileSubtype = 0;

  VersionWriter w;
  const size_t root = w.Begin(L"VS_VERSION_INFO", sizeof(ffi), 0 /*binary*/);
  w.Append(&ffi, sizeof(ffi));

  const size_t sfi = w.Begin(L"StringFileInfo", 0, 1);
  // 040904B0 = en-US, Unicode (codepage 1200) — must agree with the Translation below.
  const size_t table = w.Begin(L"040904B0", 0, 1);
  w.AddString(L"CompanyName", kCompanyName);
  w.AddString(L"FileDescription", kFileDescription);
  w.AddString(L"FileVersion", version);
  w.AddString(L"InternalName", kOriginalFilename);
  w.AddString(L"LegalCopyright", kLegalCopyright);
  w.AddString(L"OriginalFilename", kOriginalFilename);
  w.AddString(L"ProductName", kProductName);
  w.AddString(L"ProductVersion", version);
  w.End(table);
  w.End(sfi);

  const size_t vfi = w.Begin(L"VarFileInfo", 0, 1);
  const size_t var = w.Begin(L"Translation", sizeof(DWORD), 0 /*binary*/);
  w.U32(0x04B00409);  // low word = langid 0x0409, high word = codepage 0x04B0
  w.End(var);
  w.End(vfi);

  w.End(root);
  return w.buf();
}

}  // namespace

int wmain(int argc, wchar_t** argv) {
  if (argc != 4) {
    std::fprintf(stderr,
                 "usage: stamp_win_resources <target.exe> <icon.ico> <version>\n");
    return 2;
  }
  const std::wstring exe = argv[1];
  const std::wstring ico = argv[2];
  const std::wstring version = argv[3];

  std::vector<BYTE> icon_file;
  if (!ReadWholeFile(ico, &icon_file) || icon_file.size() < sizeof(IconDir)) {
    std::fprintf(stderr, "stamp_win_resources: cannot read icon file\n");
    return 1;
  }

  IconDir dir{};
  std::memcpy(&dir, icon_file.data(), sizeof(dir));
  if (dir.reserved != 0 || dir.type != 1 || dir.count == 0) {
    std::fprintf(stderr, "stamp_win_resources: not a valid .ico\n");
    return 1;
  }
  const size_t entries_size = static_cast<size_t>(dir.count) * sizeof(IconDirEntry);
  if (icon_file.size() < sizeof(IconDir) + entries_size) {
    std::fprintf(stderr, "stamp_win_resources: truncated .ico directory\n");
    return 1;
  }

  std::vector<IconDirEntry> entries(dir.count);
  std::memcpy(entries.data(), icon_file.data() + sizeof(IconDir), entries_size);
  for (const auto& e : entries) {
    if (static_cast<size_t>(e.image_offset) + e.bytes_in_res > icon_file.size()) {
      std::fprintf(stderr, "stamp_win_resources: .ico image out of bounds\n");
      return 1;
    }
  }

  // Snapshot CEF's icon resources before opening the update handle — the module
  // cannot be loaded for reading while an update is pending.
  const std::vector<ResourceKey> old_icons = ListResources(exe, RT_ICON);
  const std::vector<ResourceKey> old_groups = ListResources(exe, RT_GROUP_ICON);

  // FALSE: keep everything we do not explicitly touch, above all RT_MANIFEST.
  HANDLE upd = BeginUpdateResourceW(exe.c_str(), FALSE);
  if (!upd) {
    Fail("BeginUpdateResource failed");
    return 1;
  }

  auto abort_with = [&](const char* msg) {
    Fail(msg);
    EndUpdateResourceW(upd, TRUE /*discard*/);
    return 1;
  };

  // Drop CEF's icons so ours is unambiguously the only one.
  for (const auto& k : old_groups) {
    if (!UpdateResourceW(upd, RT_GROUP_ICON, k.NamePtr(), k.lang, nullptr, 0))
      return abort_with("failed deleting an existing RT_GROUP_ICON");
  }
  for (const auto& k : old_icons) {
    if (!UpdateResourceW(upd, RT_ICON, k.NamePtr(), k.lang, nullptr, 0))
      return abort_with("failed deleting an existing RT_ICON");
  }

  // Images become RT_ICON 1..N; the group that references them becomes id 1.
  std::vector<GrpIconDirEntry> group(dir.count);
  for (WORD i = 0; i < dir.count; ++i) {
    const IconDirEntry& e = entries[i];
    const WORD id = static_cast<WORD>(i + 1);
    if (!UpdateResourceW(upd, RT_ICON, MAKEINTRESOURCEW(id), kLangEnUs,
                         icon_file.data() + e.image_offset, e.bytes_in_res))
      return abort_with("UpdateResource(RT_ICON) failed");

    group[i].width = e.width;
    group[i].height = e.height;
    group[i].color_count = e.color_count;
    group[i].reserved = e.reserved;
    group[i].planes = e.planes;
    group[i].bit_count = e.bit_count;
    group[i].bytes_in_res = e.bytes_in_res;
    group[i].id = id;
  }

  std::vector<BYTE> group_blob(sizeof(IconDir) + group.size() * sizeof(GrpIconDirEntry));
  IconDir gdir{0, 1, dir.count};
  std::memcpy(group_blob.data(), &gdir, sizeof(gdir));
  std::memcpy(group_blob.data() + sizeof(gdir), group.data(),
              group.size() * sizeof(GrpIconDirEntry));

  if (!UpdateResourceW(upd, RT_GROUP_ICON, MAKEINTRESOURCEW(1), kLangEnUs,
                       group_blob.data(), static_cast<DWORD>(group_blob.size())))
    return abort_with("UpdateResource(RT_GROUP_ICON) failed");

  std::vector<BYTE> ver = BuildVersionResource(version);
  if (!UpdateResourceW(upd, RT_VERSION, MAKEINTRESOURCEW(VS_VERSION_INFO), kLangEnUs,
                       ver.data(), static_cast<DWORD>(ver.size())))
    return abort_with("UpdateResource(RT_VERSION) failed");

  if (!EndUpdateResourceW(upd, FALSE)) {
    Fail("EndUpdateResource failed");
    return 1;
  }

  std::printf("stamped %d icon image(s) + VERSIONINFO into the target exe\n", dir.count);
  return 0;
}
