/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-sysdeps-wince-glue.h Emulation of system/libc features for Windows CE (internal to D-Bus implementation)
 *
 * Copyright (C) 2002, 2003  Red Hat, Inc.
 * Copyright (C) 2003 CodeFactory AB
 *
 * Licensed under the Academic Free License version 2.1
 *
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; either version 2 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program; if not, write to the Free Software
 * Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301  USA
 *
 */

#ifndef DBUS_SYSDEPS_WINCE_GLUE_H
#define DBUS_SYSDEPS_WINCE_GLUE_H

#include <time.h>
#include <stdarg.h>

/* For getaddrinfo, configure/cmake defined _WIN32_WCE to something >= 0x0401.  */
#include <windows.h>
#undef interface

DBUS_BEGIN_DECLS

/* shlobj.h declares these only for _WIN32_IE that we don't want to define.
   In any case, with mingw32ce we only get a SHGetSpecialFolderPath.  */
#define SHGetSpecialFolderPathW SHGetSpecialFolderPath
BOOL WINAPI SHGetSpecialFolderPathA(HWND,LPSTR,int,BOOL);
BOOL WINAPI SHGetSpecialFolderPathW(HWND,LPWSTR,int,BOOL);

#ifndef TLS_OUT_OF_INDEXES
#define TLS_OUT_OF_INDEXES 0xffffffff
#endif


/* Seriously.  Windows CE does not have errno.  Don't you hate it when
   that happens?  */
#define errno ((int)GetLastError ())

#define ENOENT          ERROR_FILE_NOT_FOUND
#define EMFILE          ERROR_TOO_MANY_OPEN_FILES
#define EACCES          ERROR_ACCESS_DENIED
#define EBADF           ERROR_INVALID_HANDLE
#define ENOMEM          ERROR_NOT_ENOUGH_MEMORY
#define EXDEV           ERROR_NOT_SAME_DEVICE
#define ENFILE          ERROR_NO_MORE_FILES
#define EROFS           ERROR_WRITE_PROTECT
#define ENOLCK          ERROR_SHARING_BUFFER_EXCEEDED
#define ENOSYS          ERROR_NOT_SUPPORTED
#define EEXIST          ERROR_FILE_EXISTS
#define EPERM           ERROR_CANNOT_MAKE
#define EINVAL          ERROR_INVALID_PARAMETER
#define EINTR           ERROR_INVALID_AT_INTERRUPT_TIME
#define EPIPE           ERROR_BROKEN_PIPE
#define ENOSPC          ERROR_DISK_FULL
#define ENOTEMPTY       ERROR_DIR_NOT_EMPTY
#define EBUSY           ERROR_BUSY
#define ENAMETOOLONG    ERROR_FILENAME_EXCED_RANGE
#define EAGAIN          ERROR_MORE_DATA
#define ENOTDIR         ERROR_DIRECTORY
#define ERANGE          ERROR_ARITHMETIC_OVERFLOW
#define ENXIO           ERROR_FILE_INVALID
#define EFAULT          ERROR_PROCESS_ABORTED
#define EIO             ERROR_IO_DEVICE
#define EDEADLOCK       ERROR_POSSIBLE_DEADLOCK
#define ENODEV          ERROR_BAD_DEVICE

/* Windows CE is missing more stuff that is pretty standard.  */

#define strdup _strdup
#define stricmp _stricmp
#define strnicmp _strnicmp

#define environ _dbus_wince_environ
extern char *environ[];

#define getenv _dbus_wince_getenv
char *getenv (const char *name);

#define putenv _dbus_wince_putenv
int putenv (char *str);

#define clock _dbus_wince_clock
clock_t clock (void);

#define abort _dbus_wince_abort
void abort (void);

#define _S_IFMT         0170000         /* file type mask */
#define _S_IFDIR        0040000         /* directory */
#define _S_IFCHR        0020000         /* character special */
#define _S_IFIFO        0010000         /* pipe */
#define _S_IFREG        0100000         /* regular */
#define _S_IREAD        0000400         /* read permission, owner */
#define _S_IWRITE       0000200         /* write permission, owner */
#define _S_IEXEC        0000100         /* execute/search permission, owner */

#ifndef __OFF_T_DEFINED
typedef long off_t;
#define __OFF_T_DEFINED
#endif
#ifndef _INTPTR_T_DEFINED
typedef int intptr_t;
#define _INTPTR_T_DEFINED
#endif
#ifndef _UINTPTR_T_DEFINED
typedef unsigned int uintptr_t;
#define _UINTPTR_T_DEFINED
#endif

#ifndef _MAX_FNAME
#define _MAX_FNAME 256
#endif

#ifndef _IOFBF
#define _IOFBF	0
#endif
#ifndef _IOLBF
#define _IOLBF	1
#endif
#ifndef _IONBF
#define _IONBF	2
#endif


/* Windows CE is missing some Windows functions that we want.  */

#define GetSystemTimeAsFileTime _dbus_wince_GetSystemTimeAsFileTime
void GetSystemTimeAsFileTime (LPFILETIME ftp);

#define _mbsrchr _dbus_wince_mbsrchr
unsigned char* _mbsrchr (const unsigned char*, unsigned int);

#define OpenFileMappingA _dbus_wince_OpenFileMappingA
HANDLE OpenFileMappingA(DWORD,BOOL,LPCSTR);

#define MoveFileExA _dbus_wince_MoveFileExA
BOOL MoveFileExA(LPCSTR,LPCSTR,DWORD);
#ifndef MOVEFILE_REPLACE_EXISTING
#define MOVEFILE_REPLACE_EXISTING 0x00000001
#endif

#define SetHandleInformation _dbus_wince_SetHandleInformation
BOOL SetHandleInformation(HANDLE,DWORD,DWORD);
#ifndef HANDLE_FLAG_INHERIT
#define HANDLE_FLAG_INHERIT 0x01
#endif
#ifndef HANDLE_FLAG_PROTECT
#define HANDLE_FLAG_PROTECT_FROM_CLOSE 0x02
#endif

#define SearchPathA _dbus_wince_SearchPathA
DWORD SearchPathA(LPCSTR,LPCSTR,LPCSTR,DWORD,LPSTR,LPSTR*);

/* Instead of emulating all functions needed for this, we replace the
   whole thing.  */
dbus_bool_t _dbus_getsid(char **sid);


#define LookupAccountNameW _dbus_wince_LookupAccountNameW
BOOL LookupAccountNameW(LPCWSTR,LPCWSTR,PSID,PDWORD,LPWSTR,PDWORD,PSID_NAME_USE);

#define IsValidSid _dbus_wince_IsValidSid
BOOL IsValidSid(PSID);


/* Windows CE does only have the UNICODE interfaces (FooW), but we
   want to use the ASCII interfaces (FooA).  We implement them
   here.  */

#define CreateFileA _dbus_wince_CreateFileA
HANDLE CreateFileA(LPCSTR,DWORD,DWORD,LPSECURITY_ATTRIBUTES,DWORD,DWORD,HANDLE);

#define DeleteFileA _dbus_wince_DeleteFileA
BOOL DeleteFileA(LPCSTR);

#define GetFileAttributesA _dbus_wince_GetFileAttributesA
DWORD GetFileAttributesA(LPCSTR);

#define GetFileAttributesExA _dbus_wince_GetFileAttributesExA
BOOL GetFileAttributesExA(LPCSTR,GET_FILEEX_INFO_LEVELS,PVOID);

#define CreateFileMappingA _dbus_wince_CreateFileMappingA
HANDLE CreateFileMappingA(HANDLE,LPSECURITY_ATTRIBUTES,DWORD,DWORD,DWORD,LPCSTR);

#define CreateDirectoryA _dbus_wince_CreateDirectoryA
BOOL CreateDirectoryA(LPCSTR,LPSECURITY_ATTRIBUTES);

#define RemoveDirectoryA _dbus_wince_RemoveDirectoryA
BOOL RemoveDirectoryA(LPCSTR);

#define FindFirstFileA _dbus_wince_FindFirstFileA
HANDLE FindFirstFileA(LPCSTR,LPWIN32_FIND_DATAA);

#define FindNextFileA _dbus_wince_FindNextFileA
BOOL FindNextFileA(HANDLE,LPWIN32_FIND_DATAA);

#define CreateMutexA _dbus_wince_CreateMutexA
HANDLE CreateMutexA(LPSECURITY_ATTRIBUTES,BOOL,LPCSTR);

#define CreateProcessA _dbus_wince_CreateProcessA
BOOL CreateProcessA(LPCSTR,LPSTR,LPSECURITY_ATTRIBUTES,LPSECURITY_ATTRIBUTES,BOOL,DWORD,PVOID,LPCSTR,LPSTARTUPINFOA,LPPROCESS_INFORMATION);
#ifndef CREATE_NO_WINDOW
#define CREATE_NO_WINDOW 0x08000000
#endif


#define RegOpenKeyExA _dbus_wince_RegOpenKeyExA
LONG RegOpenKeyExA(HKEY,LPCSTR,DWORD,REGSAM,PHKEY);

#define RegQueryValueExA _dbus_wince_RegQueryValueExA
LONG WINAPI RegQueryValueExA(HKEY,LPCSTR,LPDWORD,LPDWORD,LPBYTE,LPDWORD);


#define FormatMessageA _dbus_wince_FormatMessageA
DWORD FormatMessageA(DWORD,PCVOID,DWORD,DWORD,LPSTR,DWORD,va_list*);

#define GetModuleFileNameA _dbus_wince_GetModuleFileNameA
DWORD GetModuleFileNameA(HINSTANCE,LPSTR,DWORD);

#define GetTempPathA _dbus_wince_GetTempPathA
DWORD GetTempPathA(DWORD,LPSTR);

#define SHGetSpecialFolderPathA _dbus_wince_SHGetSpecialFolderPathA
BOOL SHGetSpecialFolderPathA(HWND,LPSTR,int,BOOL);


#define OutputDebugStringA _dbus_wince_OutputDebugStringA
void OutputDebugStringA(LPCSTR);


DBUS_END_DECLS

#endif /* DBUS_SYSDEPS_WINCE_GLUE_H */
