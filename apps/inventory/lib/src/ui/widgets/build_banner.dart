import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:shared_preferences/shared_preferences.dart';

class BuildMeta {
  static const sha = String.fromEnvironment('BUILD_SHA', defaultValue: 'dev');
  static const time = String.fromEnvironment('BUILD_TIME', defaultValue: 'local');
  static const seed = String.fromEnvironment('BUILD_SEED', defaultValue: '0');

  static Color color() {
    // Deterministic color from seed (e.g., commit hash)
    final s = seed.isEmpty ? sha : seed;
    int h = 0;
    for (final c in s.codeUnits) {
      h = (h * 31 + c) & 0x7FFFFFFF;
    }
    final hue = (h % 360).toDouble();
    return HSLColor.fromAHSL(0.85, hue, 0.60, 0.55).toColor();
  }
}

class BuildBanner extends StatefulWidget {
  final Widget child;
  final bool show;

  const BuildBanner({
    super.key,
    required this.child,
    this.show = !kReleaseMode,
  });

  @override
  State<BuildBanner> createState() => _BuildBannerState();
}

class _BuildBannerState extends State<BuildBanner> {
  bool _isVisible = true;
  static const _lastBuildShaKey = 'last_build_sha';

  @override
  void initState() {
    super.initState();
    if (widget.show) {
      _checkBuildSha();
    }
  }

  Future<void> _checkBuildSha() async {
    final prefs = await SharedPreferences.getInstance();
    final lastSha = prefs.getString(_lastBuildShaKey);
    final currentSha = BuildMeta.sha;

    if (lastSha != currentSha && currentSha != 'dev') {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(
              content: Text('New build detected: $currentSha'),
              backgroundColor: BuildMeta.color(),
              duration: const Duration(seconds: 4),
            ),
          );
        }
      });
      await prefs.setString(_lastBuildShaKey, currentSha);
    }
  }

  @override
  Widget build(BuildContext context) {
    if (!widget.show) return widget.child;
    return Stack(
      children: [
        widget.child,
        AnimatedOpacity(
          opacity: _isVisible ? 1.0 : 0.0,
          duration: const Duration(milliseconds: 300),
          child: _isVisible
              ? Positioned(
                  top: 8,
                  left: 8,
                  right: 8,
                  child: GestureDetector(
                    onTap: () {
                      setState(() {
                        _isVisible = false;
                      });
                    },
                    child: SafeArea(
                      child: Container(
                        padding: const EdgeInsets.symmetric(
                            horizontal: 10, vertical: 6),
                        decoration: BoxDecoration(
                          color: BuildMeta.color(),
                          borderRadius: BorderRadius.circular(12),
                        ),
                        child: DefaultTextStyle(
                          style: const TextStyle(
                            fontSize: 12,
                            color: Colors.white,
                            fontWeight: FontWeight.w700,
                          ),
                          child: Row(
                            mainAxisAlignment: MainAxisAlignment.spaceBetween,
                            children: [
                              const Text('BUILD'),
                              Text('${BuildMeta.sha} • ${BuildMeta.time}'),
                            ],
                          ),
                        ),
                      ),
                    ),
                  ),
                )
              : const SizedBox.shrink(),
        ),
      ],
    );
  }
}
