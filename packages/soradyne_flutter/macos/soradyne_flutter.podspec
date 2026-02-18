Pod::Spec.new do |s|
  s.name             = 'soradyne_flutter'
  s.version          = '0.1.0'
  s.summary          = 'Flutter FFI plugin for Soradyne'
  s.description      = 'FFI plugin wrapping soradyne_core. The dylib is loaded directly by Dart; this podspec is a required stub for CocoaPods integration.'
  s.homepage         = 'https://github.com/soradyne'
  s.license          = { :type => 'MIT' }
  s.author           = { 'rim' => 'dev@soradyne.io' }
  s.source           = { :path => '.' }
  s.source_files     = 'Classes/**/*'
  s.dependency 'FlutterMacOS'

  s.platform = :osx, '10.14'
  s.pod_target_xcconfig = { 'DEFINES_MODULE' => 'YES' }
  s.swift_version = '5.0'
end
