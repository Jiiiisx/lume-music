Pod::Spec.new do |s|
  s.name           = 'LumeCore'
  s.version        = '1.0.0'
  s.summary        = 'Lume Music Rust Core (UniFFI) + iOS Remote Media Controls bridge'
  s.description    = 'Exposes lume-core, lume-storage, and lume-providers to Expo React Native via UniFFI XCFramework.'
  s.author         = 'Lume Music'
  s.homepage       = 'https://github.com/Jiiiisx/lume-music'
  s.platforms      = { :ios => '15.1' }
  s.source         = { :git => '' }
  s.static_framework = true

  s.dependency 'ExpoModulesCore'

  s.vendored_frameworks = 'Frameworks/LumeFfi.xcframework'
  s.frameworks = ['Security', 'SystemConfiguration', 'MediaPlayer', 'AVFoundation']
  s.libraries = ['z', 'c++']

  s.pod_target_xcconfig = {
    'DEFINES_MODULE' => 'YES',
    'SWIFT_COMPILATION_MODE' => 'wholemodule'
  }

  s.source_files = '*.swift', 'Generated/**/*.swift'
end
