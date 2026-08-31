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
  s.license          = { :type => 'MIT', :file => 'LICENSE' }
  s.author           = { 'yearsyan' => 'yearsyan@hotmail.com' }
  # The zip is the pod root: build/XPatchlib.xcframework + Module.modulemap
  # + LICENSE, assembled by build-xcframework.sh and attached to the tag's
  # GitHub Release. :sha256 is the checksum of the actual release asset.
  s.source           = {
    :http => 'https://github.com/yearsyan/xpatchlib/releases/download/v0.1.0/XPatchlib.xcframework.zip',
    :sha256 => '06a95c2cda3eead13a49e4dc6112be1cfd44567a10b7e58c42c3331a408d2b1c',
  }
  s.ios.deployment_target = '13.0'
  s.vendored_frameworks = 'build/XPatchlib.xcframework'
  # Exposes the C header to Swift via an explicit module map.
  s.module_map = 'Module.modulemap'
  s.preserve_paths = 'build/XPatchlib.xcframework'
end
