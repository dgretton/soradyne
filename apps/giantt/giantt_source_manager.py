#!/usr/bin/env python3
"""
Giantt Source Manager

A specialized version of source_manager.py configured for the Giantt project.
This script collects all Giantt-related source code from across the monorepo
into a single concatenated file for LLM analysis.

Usage:
    ./giantt_source_manager.py init                    # Create default config
    ./giantt_source_manager.py generate               # Generate concatenated file
    ./giantt_source_manager.py add <file>             # Add specific file
    ./giantt_source_manager.py exclude <file>         # Exclude file from collection
    ./giantt_source_manager.py list                   # Show included files
"""

import os
import sys
import glob
import argparse
import json
from datetime import datetime
from pathlib import Path
from typing import Dict, List, Set, Optional, Any

# The script lives at apps/giantt/giantt_source_manager.py
# So the monorepo root is two levels up from the script file itself.
SCRIPT_DIR = Path(__file__).resolve().parent          # apps/giantt/
MONOREPO_ROOT = SCRIPT_DIR.parent.parent              # <repo root>/
GIANTT_CORE_DIR = MONOREPO_ROOT / "packages" / "giantt_core"
PYTHON_REF_FILE = MONOREPO_ROOT / "docs" / "port_reference" / "giantt_core.py"


class GianttSourceManager:
    def __init__(self, source_dir=None, output_file=None, config_file=None):
        # source_dir is the apps/giantt directory
        self.source_dir = Path(source_dir).resolve() if source_dir else SCRIPT_DIR
        self.config_file = config_file or self.source_dir / '.source_manager' / 'config.json'
        self.config = self._load_config()
        self.output_file = output_file or self.config.get('output_file', 'giantt_complete.txt')
        # Make output_file absolute so we can write it regardless of cwd
        if not Path(self.output_file).is_absolute():
            self.output_file = str(self.source_dir / self.output_file)

        # File collections by category
        # Each entry is a dict: {'display': <relative label>, 'full': <absolute Path>}
        self.file_collections = {category: [] for category in self.config['file_types'].keys()}

        # Exclusion and priority file management
        self.exclusions_dir = self.source_dir / '.source_manager'
        self.exclusions_file = self.exclusions_dir / 'exclusions.txt'
        self.priority_file = self.exclusions_dir / 'priority.txt'
        self._ensure_exclusions_setup()
        self._ensure_priority_setup()
        self.individual_exclusions = self._load_exclusions()
        self.priority_files = self._load_priority_files()

    # ------------------------------------------------------------------
    # Config helpers
    # ------------------------------------------------------------------

    def _load_config(self) -> Dict[str, Any]:
        """Load configuration from JSON file or create default."""
        if self.config_file.exists():
            try:
                with open(self.config_file, 'r') as f:
                    return json.load(f)
            except (json.JSONDecodeError, FileNotFoundError):
                print(f"Warning: Could not load config from {self.config_file}, using defaults")
        return self._default_config()

    def _default_config(self) -> Dict[str, Any]:
        """Return default configuration for Giantt project."""
        return {
            "project_name": "Giantt Project",
            "output_file": "giantt_complete.txt",
            "file_types": {
                "dart_source": {
                    "extensions": [".dart"],
                    "description": "Dart source code files"
                },
                "flutter_config": {
                    "extensions": [".yaml", ".yml"],
                    "description": "Flutter and Dart configuration files"
                },
                "android_config": {
                    "extensions": [".kt", ".properties", ".gradle"],
                    "description": "Android configuration files"
                },
                "python_reference": {
                    "extensions": [".py"],
                    "description": "Python reference implementation"
                },
                "documentation": {
                    "extensions": [".md", ".rst", ".txt"],
                    "description": "Documentation files"
                }
            },
            "excluded_dirs": [
                ".git", ".dart_tool", "build", ".gradle",
                "ios", ".idea", ".vscode", "coverage", ".packages",
                ".source_manager"
            ],
            "banner_config": {
                "padding_h": 5,
                "padding_v": 1,
                "char": "="
            }
        }

    def _save_config(self):
        """Save current configuration to file."""
        self.exclusions_dir.mkdir(parents=True, exist_ok=True)
        with open(self.config_file, 'w') as f:
            json.dump(self.config, f, indent=2)

    # ------------------------------------------------------------------
    # Exclusions / priority setup
    # ------------------------------------------------------------------

    def _ensure_exclusions_setup(self):
        """Ensure the .source_manager directory and exclusions file exist."""
        self.exclusions_dir.mkdir(parents=True, exist_ok=True)
        if not self.exclusions_file.exists():
            with open(self.exclusions_file, 'w') as f:
                f.write("# Individual file exclusions for giantt_source_manager\n")
                f.write("# One file path per line, relative to the directory being scanned\n")
                f.write("# Lines starting with # are comments and will be ignored\n\n")
                f.write("# Common output files\n")
                f.write("giantt_complete.txt\n")
                f.write("*.log\n")
                f.write("*.tmp\n")
                f.write("\n# Generated files\n")
                f.write("android/app/src/main/java/io/flutter/plugins/GeneratedPluginRegistrant.java\n")
                f.write("android/local.properties\n")
                f.write(".flutter-plugins\n")
                f.write(".flutter-plugins-dependencies\n")
                f.write(".packages\n")
                f.write("pubspec.lock\n")

    def _ensure_priority_setup(self):
        """Ensure the priority files file exists."""
        self.exclusions_dir.mkdir(parents=True, exist_ok=True)
        if not self.priority_file.exists():
            with open(self.priority_file, 'w') as f:
                f.write("# Priority files for giantt_source_manager\n")
                f.write("# These files will always be included in the output\n")
                f.write("# Paths are relative to the apps/giantt directory OR absolute\n")
                f.write("# Lines starting with # are comments and will be ignored\n\n")

    def _load_exclusions(self):
        """Load the list of excluded files."""
        if not self.exclusions_file.exists():
            return []
        with open(self.exclusions_file, 'r') as f:
            return [line.strip() for line in f if line.strip() and not line.startswith('#')]

    def _load_priority_files(self):
        """Load the list of priority files."""
        if not self.priority_file.exists():
            return []
        with open(self.priority_file, 'r') as f:
            return [line.strip() for line in f if line.strip() and not line.startswith('#')]

    # ------------------------------------------------------------------
    # Exclusion checking
    # ------------------------------------------------------------------

    def _is_excluded(self, file_name: str) -> bool:
        """Check if a filename (or relative path fragment) matches any exclusion pattern."""
        import fnmatch
        # Always exclude the output file itself
        if Path(file_name).name == Path(self.output_file).name:
            return True
        for exclusion in self.individual_exclusions:
            if file_name == exclusion:
                return True
            if '*' in exclusion or '?' in exclusion:
                if fnmatch.fnmatch(file_name, exclusion) or fnmatch.fnmatch(Path(file_name).name, exclusion):
                    return True
            else:
                if Path(file_name).name == exclusion:
                    return True
        return False

    # ------------------------------------------------------------------
    # File categorisation
    # ------------------------------------------------------------------

    def _categorize_and_add_file(self, display_path: str, full_path: Path, force: bool = False):
        """Categorize a file by extension and add to the appropriate collection."""
        file_ext = full_path.suffix.lower()
        entry = {'display': display_path, 'full': full_path}

        for category, cfg in self.config['file_types'].items():
            if file_ext in cfg['extensions']:
                # Avoid duplicates (compare by resolved full path)
                if not any(e['full'] == full_path for e in self.file_collections[category]):
                    self.file_collections[category].append(entry)
                return

        if force:
            if 'other' not in self.file_collections:
                self.file_collections['other'] = []
                self.config['file_types']['other'] = {'extensions': [], 'description': 'Other files'}
            if not any(e['full'] == full_path for e in self.file_collections['other']):
                self.file_collections['other'].append(entry)

    # ------------------------------------------------------------------
    # Collection
    # ------------------------------------------------------------------

    def collect_files(self, debug=False):
        """Collect files from Giantt app, core package, and reference docs."""
        # Clear existing collections
        for category in self.file_collections:
            self.file_collections[category] = []

        # 1. Collect from apps/giantt (the script's own directory)
        print(f"Scanning app directory: {self.source_dir}")
        self._collect_from_directory(self.source_dir, label_root=self.source_dir, debug=debug)

        # 2. Collect from packages/giantt_core
        if GIANTT_CORE_DIR.exists():
            print(f"Scanning core package: {GIANTT_CORE_DIR}")
            self._collect_from_directory(GIANTT_CORE_DIR, label_root=MONOREPO_ROOT, debug=debug)
        else:
            print(f"WARNING: giantt_core directory not found at {GIANTT_CORE_DIR}")

        # 3. Add the Python reference file
        if PYTHON_REF_FILE.exists():
            display = str(PYTHON_REF_FILE.relative_to(MONOREPO_ROOT))
            if not self._is_excluded(PYTHON_REF_FILE.name):
                self._categorize_and_add_file(display, PYTHON_REF_FILE)
                if debug:
                    print(f"DEBUG: Added Python reference: {display}")
        else:
            print(f"WARNING: Python reference not found at {PYTHON_REF_FILE}")

        # 4. Add any extra priority files listed in priority.txt
        for pf in self.priority_files:
            p = Path(pf)
            if not p.is_absolute():
                p = (self.source_dir / pf).resolve()
            if p.exists():
                try:
                    display = str(p.relative_to(MONOREPO_ROOT))
                except ValueError:
                    display = str(p)
                if not self._is_excluded(p.name):
                    self._categorize_and_add_file(display, p, force=True)
                    if debug:
                        print(f"DEBUG: Added priority file: {display}")
            else:
                print(f"WARNING: Priority file not found: {p}")

        # Summary
        total_files = sum(len(col) for col in self.file_collections.values())
        print(f"\nFound {total_files} Giantt-related files:")
        for category, files in self.file_collections.items():
            if files:
                description = self.config['file_types'][category]['description']
                print(f"  {len(files)} {description.lower()}")
        print(f"Exclusion patterns active: {len(self.individual_exclusions)}")

        return self.file_collections

    def _collect_from_directory(self, directory: Path, label_root: Path, debug=False):
        """Walk *directory* and add matching files. Display paths are relative to *label_root*."""
        excluded_dirs = set(self.config['excluded_dirs'])

        for root, dirs, files in os.walk(directory):
            root_path = Path(root)

            # Prune excluded directories in-place
            dirs[:] = [d for d in dirs if d not in excluded_dirs]

            for file_name in files:
                full_path = root_path / file_name

                # Skip hidden files and lock files
                if file_name.startswith('.') or file_name.endswith('.lock'):
                    if debug:
                        print(f"DEBUG: Skipping hidden/lock file: {full_path}")
                    continue

                # Skip the script itself
                if full_path.resolve() == Path(__file__).resolve():
                    continue

                if self._is_excluded(file_name):
                    if debug:
                        print(f"DEBUG: Excluded: {full_path}")
                    continue

                try:
                    display = str(full_path.relative_to(label_root))
                except ValueError:
                    display = str(full_path)

                self._categorize_and_add_file(display, full_path, force=False)
                if debug:
                    print(f"DEBUG: Added: {display}")

    # ------------------------------------------------------------------
    # Output generation
    # ------------------------------------------------------------------

    def generate_concatenated_file(self, debug=False):
        """Generate a new concatenated file from all Giantt-related source files."""
        self.collect_files(debug=debug)

        output_path = Path(self.output_file)
        print(f"\nWriting output to: {output_path}")

        with open(output_path, 'w', encoding='utf-8') as output:
            total_files = sum(len(col) for col in self.file_collections.values())

            # Header banner
            project_name = self.config['project_name'].upper()
            header_text = f"{project_name} - COMPLETE SOURCE CODE"
            border_char = self.config['banner_config']['char']
            border_len = max(70, len(header_text) + 10)
            border = border_char * border_len

            output.write(f"{border}\n")
            output.write(f"{border_char} {header_text:<{border_len-4}} {border_char}\n")
            output.write(f"{border_char} Generated on: {datetime.now().strftime('%Y-%m-%d %H:%M:%S'):<{border_len-16}} {border_char}\n")
            output.write(f"{border_char} Total files: {total_files:<{border_len-15}} {border_char}\n")
            output.write(f"{border}\n\n")

            # Files by category
            for category, files in self.file_collections.items():
                if not files:
                    continue

                section_title = self.config['file_types'][category]['description'].upper()
                section_border = "=" * 70
                output.write(f"\n\n{section_border}\n")
                output.write(f"= {section_title:<66} =\n")
                output.write(f"{section_border}\n\n")

                for entry in sorted(files, key=lambda e: e['display']):
                    display_path = entry['display']
                    full_path = entry['full']

                    ext = full_path.suffix.lower()
                    if ext in ['.html', '.xml', '.md']:
                        cs, ce = "<!-- ", " -->"
                    elif ext in ['.py', '.sh', '.yaml', '.yml', '.toml', '.properties']:
                        cs, ce = "# ", ""
                    else:
                        cs, ce = "// ", ""

                    output.write(f"\n\n{cs}{'=' * 69}{ce}\n")
                    output.write(f"{cs}FILE: {display_path}{ce}\n")
                    output.write(f"{cs}{'=' * 69}{ce}\n\n")

                    try:
                        with open(full_path, 'r', encoding='utf-8') as f:
                            output.write(f.read())
                    except UnicodeDecodeError:
                        output.write(f"{cs}[Binary file – skipped]{ce}\n")
                    except FileNotFoundError:
                        output.write(f"{cs}[File not found: {full_path}]{ce}\n")

            # Footer
            footer_border = "=" * 70
            output.write(f"\n\n{footer_border}\n")
            output.write(f"= {'END OF FILES':<66} =\n")
            output.write(f"{footer_border}\n\n")

        print(f"Done! Output file: {output_path}")

    # ------------------------------------------------------------------
    # Init
    # ------------------------------------------------------------------

    def init_config(self):
        """Initialize configuration file with Giantt-specific defaults."""
        if self.config_file.exists():
            print(f"Configuration already exists at {self.config_file}")
            return False
        self.config = self._default_config()
        self._save_config()
        print(f"Created Giantt-specific configuration at {self.config_file}")
        return True


def main():
    parser = argparse.ArgumentParser(
        description='Giantt Source Manager - Collect Giantt-related source code files',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  %(prog)s init                     # Create Giantt-specific configuration
  %(prog)s generate                 # Generate concatenated source file
  %(prog)s generate --debug         # Generate with debug output

This tool collects:
  - Flutter app code from apps/giantt/
  - Core Dart package from packages/giantt_core/
  - Python reference from docs/port_reference/giantt_core.py
        """
    )

    subparsers = parser.add_subparsers(dest='command', help='Available commands')

    subparsers.add_parser('init', help='Initialize Giantt-specific configuration')

    generate_parser = subparsers.add_parser('generate', help='Generate concatenated source file')
    generate_parser.add_argument('--debug', action='store_true', help='Enable debug output')

    args = parser.parse_args()

    if not args.command:
        parser.print_help()
        return

    manager = GianttSourceManager()

    if args.command == 'init':
        manager.init_config()
    elif args.command == 'generate':
        manager.generate_concatenated_file(debug=getattr(args, 'debug', False))


if __name__ == '__main__':
    main()
