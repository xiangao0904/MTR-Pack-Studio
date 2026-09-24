# Windows project file icon

`tauri.windows.conf.json` bundles `icons/mtrpack.ico` at `icons/mtrpack.ico`
inside the installation directory. The application keeps its own `icons/icon.ico`.
The project icon source is `public/images/mtrpack-file.svg`; the packaging copy
matches `docs/design/file-icon/mtrpack.ico`. Update both ICO copies when redesigning it.

- NSIS: the post-install hook replaces the default icon on Tauri's existing
  `MTR Pack Studio Project` association, using the same installation scope.
  Tauri removes the association and resource on uninstall. Hooks notify Explorer
  after installation and uninstallation.
- MSI: a WiX component owns the DefaultIcon registry value for Tauri's generated
  `MTR Pack Studio.mtrpack` ProgId. Windows Installer removes the value on uninstall.
  Keep these ProgIds aligned with the association name and product name if renamed.

Build both formats with `pnpm tauri build --bundles nsis msi`.
For an existing build, use `pnpm tauri bundle --bundles nsis msi`.

Packaging verification: both debug installers compiled successfully; inspected
the generated NSIS resource/association/uninstall instructions and MSI registry
and file tables. Interactive installation, Explorer display, and uninstall still
need a Windows VM smoke test. Building a package does not install its association
on the development machine.

References: [Tauri installer hooks](https://v2.tauri.app/distribute/windows-installer/),
[Windows Installer ProgId table](https://learn.microsoft.com/en-us/windows/win32/msi/progid-table).
