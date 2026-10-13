/* Shortwave launcher for Windows.
 *
 * GLib's system data directories are baked to the build prefix at compile
 * time, so a relocated installation cannot find its GSettings schemas.
 * This launcher points XDG_DATA_DIRS at the bundle's share/ directory
 * (relative to the executable) and then runs the real shortwave binary.
 */
#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(void)
{
    char dir[MAX_PATH];
    char share[MAX_PATH];
    char exe[MAX_PATH];
    char env[8192];
    const char *existing;

    /* Directory containing this launcher (...\Shortwave\bin) */
    GetModuleFileNameA(NULL, dir, MAX_PATH);
    char *slash = strrchr(dir, '\\');
    if (slash)
        *slash = '\0';

    /* <installdir>\share */
    _snprintf(share, MAX_PATH, "%s\\..\\share", dir);
    GetFullPathNameA(share, MAX_PATH, share, NULL);

    /* Prepend bundle share to any existing XDG_DATA_DIRS */
    existing = getenv("XDG_DATA_DIRS");
    if (existing && existing[0])
        _snprintf(env, sizeof(env), "XDG_DATA_DIRS=%s;%s", share, existing);
    else
        _snprintf(env, sizeof(env), "XDG_DATA_DIRS=%s", share);
    putenv(env);

    _snprintf(exe, MAX_PATH, "%s\\shortwave-bin.exe", dir);

    STARTUPINFOA si = { sizeof(si) };
    PROCESS_INFORMATION pi;
    if (!CreateProcessA(exe, GetCommandLineA(), NULL, NULL, TRUE, 0,
                        NULL, dir, &si, &pi)) {
        char msg[512];
        _snprintf(msg, sizeof(msg),
                  "Failed to start shortwave-bin.exe (error %lu)",
                  GetLastError());
        MessageBoxA(NULL, msg, "Shortwave", MB_OK | MB_ICONERROR);
        return 1;
    }

    WaitForSingleObject(pi.hProcess, INFINITE);
    DWORD code = 1;
    GetExitCodeProcess(pi.hProcess, &code);
    CloseHandle(pi.hProcess);
    CloseHandle(pi.hThread);
    return (int)code;
}
