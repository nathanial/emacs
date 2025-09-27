/* Portable API for dynamic loading.

Copyright 2015-2025 Free Software Foundation, Inc.

This file is part of GNU Emacs.

GNU Emacs is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or (at
your option) any later version.

GNU Emacs is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with GNU Emacs.  If not, see <https://www.gnu.org/licenses/>.  */

#include <config.h>

#include "dynlib.h"
#include "rust-dynlib.h"

dynlib_handle_ptr
dynlib_open (const char *path)
{
  return emacs_rust_dynlib_open (path);
}

#ifdef HAVE_NATIVE_COMP
dynlib_handle_ptr
dynlib_open_for_eln (const char *path)
{
  return emacs_rust_dynlib_open_for_eln (path);
}
#endif

int
dynlib_close (dynlib_handle_ptr handle)
{
  return emacs_rust_dynlib_close (handle);
}

const char *
dynlib_error (void)
{
  return emacs_rust_dynlib_error ();
}

void *
dynlib_sym (dynlib_handle_ptr handle, const char *symbol)
{
  return emacs_rust_dynlib_sym (handle, symbol);
}

dynlib_function_ptr
dynlib_func (dynlib_handle_ptr handle, const char *symbol)
{
  return emacs_rust_dynlib_func (handle, symbol);
}

void
dynlib_addr (void (*ptr) (void), const char **file, const char **sym)
{
  emacs_rust_dynlib_addr (ptr, file, sym);
}
