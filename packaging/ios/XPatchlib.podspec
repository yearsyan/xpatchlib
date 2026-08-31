Pod::Spec.new do |s|
  s.name             = 'XPatchlib'
  s.version          = '0.1.0'
  s.summary          = 'Deterministic binary delta patch replay for app update bundles (XPDL format).'
  s.description      = <<-DESC
  Replay XPDL delta patches produced by the build toolchain. Replay only:
  the xcframework is built from xpatchlib-ffi without the "produce"
  feature, so no patch generation code ships to devices. Same xpatchlib
  core as the Android AAR and the HarmonyOS ohpm package.
  DESC
  s.homepage         = 'https://github.com/yearsyan/xpatchlib'
  s.license          = { :type => 'MIT' }
  s.author           = { 'yearsyan' => 'yearsyan@hotmail.com' }
  s.ios.deployment_target = '13.0'
  s.vendored_frameworks = 'build/XPatchlib.xcframework'
  # Exposes the C header to Swift via an explicit module map.
  s.module_map = 'Module.modulemap'
  s.preserve_paths = 'build/XPatchlib.xcframework'
end
