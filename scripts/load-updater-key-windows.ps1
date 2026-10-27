# Dot-source this file (don't run it): sets the Tauri updater signing key and its password
# from Windows Credential Manager (generic credentials tauri-updater-vee / tauri-updater-vee-pw).
# Used by scripts/release-windows.ps1; dot-source it by hand before a manual `yarn tauri build`.
# One-time setup: `. scripts/load-updater-key-windows.ps1 -Save` prompts for both values and
# stores them (cmdkey can't, its /pass is too short for the key).
param([switch]$Save)

if (-not ("VeeCredential" -as [type])) {
  Add-Type @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using FILETIME = System.Runtime.InteropServices.ComTypes.FILETIME;

public static class VeeCredential {
  [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
  struct CREDENTIAL {
    public int Flags; public int Type; public string TargetName; public string Comment;
    public FILETIME LastWritten; public int CredentialBlobSize; public IntPtr CredentialBlob;
    public int Persist; public int AttributeCount; public IntPtr Attributes;
    public string TargetAlias; public string UserName;
  }
  const int CRED_TYPE_GENERIC = 1, CRED_PERSIST_LOCAL_MACHINE = 2;

  [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
  static extern bool CredRead(string target, int type, int flags, out IntPtr cred);
  [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
  static extern bool CredWrite(ref CREDENTIAL cred, int flags);
  [DllImport("advapi32.dll")]
  static extern void CredFree(IntPtr cred);

  public static string Read(string target) {
    IntPtr p;
    if (!CredRead(target, CRED_TYPE_GENERIC, 0, out p))
      throw new Win32Exception(Marshal.GetLastWin32Error(), "Credential Manager has no " + target);
    try {
      var c = (CREDENTIAL)Marshal.PtrToStructure(p, typeof(CREDENTIAL));
      return Marshal.PtrToStringUni(c.CredentialBlob, c.CredentialBlobSize / 2);
    } finally { CredFree(p); }
  }

  public static void Write(string target, string user, string secret) {
    var c = new CREDENTIAL {
      Type = CRED_TYPE_GENERIC, TargetName = target, UserName = user, Persist = CRED_PERSIST_LOCAL_MACHINE,
      CredentialBlobSize = secret.Length * 2, CredentialBlob = Marshal.StringToCoTaskMemUni(secret),
    };
    try {
      if (!CredWrite(ref c, 0))
        throw new Win32Exception(Marshal.GetLastWin32Error(), "could not store " + target);
    } finally { Marshal.ZeroFreeCoTaskMemUnicode(c.CredentialBlob); }
  }
}
'@
}

if ($Save) {
  foreach ($target in "tauri-updater-vee", "tauri-updater-vee-pw") {
    $secret = Read-Host "$target" -AsSecureString
    [VeeCredential]::Write($target, $env:USERNAME, [Net.NetworkCredential]::new("", $secret).Password)
  }
}

$env:TAURI_SIGNING_PRIVATE_KEY = [VeeCredential]::Read("tauri-updater-vee")
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = [VeeCredential]::Read("tauri-updater-vee-pw")
