import 'package:flutter/material.dart';

import 'feature.dart';

void main() => runApp(const IncrementalApp());

class IncrementalApp extends StatelessWidget {
  const IncrementalApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      home: Scaffold(
        body: Center(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Image.asset('assets/images/logo.png', width: 64, height: 64),
              Text(
                'flavor: $buildFlavor',
                style: const TextStyle(fontFamily: 'RobotoMedium'),
              ),
              FutureBuilder<int>(
                future: bumpLaunchCount(),
                builder: (context, snapshot) =>
                    Text('launches: ${snapshot.data ?? '…'}'),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
