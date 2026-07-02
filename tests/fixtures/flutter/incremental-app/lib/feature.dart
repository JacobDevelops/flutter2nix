import 'package:shared_preferences/shared_preferences.dart';

/// Dart-define consumed at runtime — proves dartDefines key the AOT tier.
const buildFlavor = String.fromEnvironment('BUILD_FLAVOR', defaultValue: 'none');

/// Touches the shared_preferences plugin's native code path.
Future<int> bumpLaunchCount() async {
  final prefs = await SharedPreferences.getInstance();
  final count = (prefs.getInt('launchCount') ?? 0) + 1;
  await prefs.setInt('launchCount', count);
  return count;
}
