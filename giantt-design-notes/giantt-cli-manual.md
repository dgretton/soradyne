# Giantt CLI Manual

Giantt is a command-line utility for managing task dependencies and life planning through a giant Gantt chart system. This manual covers all available commands and their usage.

## Table of Contents
- [Installation](#installation)
- [Basic Usage](#basic-usage)
- [Commands](#commands)
  - [init](#init)
  - [add](#add)
  - [remove](#remove)
  - [show](#show)
  - [modify](#modify)
  - [set-status](#set-status)
  - [sort](#sort)
  - [insert](#insert)
  - [occlude](#occlude)
  - [log](#log)
  - [doctor](#doctor)
  - [touch](#touch)
  - [clean](#clean)

## Installation

Initialize Giantt in your home directory:

```bash
giantt init
```

For development environments:

```bash
giantt init --dev
```

## Basic Usage

Most commands accept a `-f` flag to specify an alternate items file (default: `~/.giantt/include/items.txt`) and `-a` for an alternate occlude file.

### Common Patterns

- Item IDs should be lowercase with underscores
- Durations use units: s (seconds), min (minutes), h/hr (hours), d (days), w (weeks), mo (months), y (years)
- Tags should be lowercase, comma-separated
- Charts are quoted strings in curly braces

## Commands

### init
Initialize the Giantt directory structure and files.

```bash
giantt init [--dev] [--data-dir PATH]
```

Options:
- `--dev`: Initialize in current directory for development
- `--data-dir`: Specify custom data directory location

### add
Add a new item to the Giantt chart.

```bash
giantt add ID "TITLE" [OPTIONS]
```

Options:
- `--duration`: Duration (e.g., "1d", "2w", "3mo")
- `--priority`: LOWEST|LOW|NEUTRAL|UNSURE|MEDIUM|HIGH|CRITICAL
- `--charts`: Comma-separated list of chart names
- `--tags`: Comma-separated list of tags
- `--status`: NOT_STARTED|IN_PROGRESS|BLOCKED|COMPLETED
- `--requires`: Comma-separated list of item IDs that this item requires
- `--any-of`: Comma-separated list of item IDs where any one is sufficient for this item

> **Note:** `blocks` and `sufficient` relations are normally added automatically along with their complements, `requires` and `any-of`, respectively. To edit manually, use the `modify` command. Other relations, such as `supercharges` etc., can only be added via the modify command at this time.

Example:
```bash
giantt add learn_python "Learn Python basics" --duration 3mo --priority HIGH --charts "Programming,Education" --tags beginner,coding --requires git_basics
```

### remove
Remove an item from the Giantt chart and clean up relations.

```bash
giantt remove ITEM_ID [OPTIONS]
```

Options:
- `--force, -f`: Remove without confirmation prompt
- `--keep-relations`: Keep relations to other items

Example:
```bash
giantt remove outdated_task
giantt remove -f unnecessary_task --keep-relations
```

### show
Show details of an item matching a substring.

```bash
giantt show SUBSTRING [OPTIONS]
```

Options:
- `--file, -f`: Specify items file
- `--occlude-file, -a`: Specify occlude items file
- `--log-file, -l`: Specify log file
- `--occlude-log-file, -al`: Specify occlude log file
- `--chart`: Search in chart names
- `--log`: Search in logs and log sessions

Example:
```bash
giantt show python
giantt show --chart "Programming"
giantt show --log dev0
```

### modify
Modify any property of a Giantt item or its relations.

```bash
giantt modify [OPTIONS] SUBSTRING PROPERTY VALUE
giantt modify [OPTIONS] SUBSTRING --add RELATION TARGET
giantt modify [OPTIONS] SUBSTRING --remove RELATION TARGET
```

Options:
- `--file, -f`: Specify items file
- `--occlude-file, -a`: Specify occlude items file 
- `--add`: Add a relation
- `--remove`: Remove a relation

Properties for standard modification:
- `title`: The item's display title
- `duration`: Duration in format like '1d', '2w', '3mo'
- `priority`: One of LOWEST, LOW, NEUTRAL, UNSURE, MEDIUM, HIGH, CRITICAL
- `status`: One of NOT_STARTED, IN_PROGRESS, BLOCKED, COMPLETED
- `charts`: Comma-separated list of chart names
- `tags`: Comma-separated list of tags

Relation types for `--add`/`--remove`:
- `requires`: Dependencies that must be completed first
- `blocks`: Items blocked by this item
- `anyof`: Any of these items enable this item
- `supercharges`: Optional enhancements
- `indicates`: Natural progressions
- `together`: Combinations
- `conflicts`: Resource conflicts
- `sufficient`: This item is sufficient for target items

Examples:
```bash
# Modify basic properties
giantt modify python duration 2w
giantt modify thesis priority CRITICAL

# Add relations
giantt modify learn_python --add blocks django
giantt modify thesis --add requires data_analysis
giantt modify startup_mvp --add together ui_design

# Remove relations
giantt modify learn_python --remove blocks django
giantt modify thesis --remove requires data_analysis

# Multiple targets in one command
giantt modify learn_python --add blocks "django,flask,fastapi"
```

### set-status
Update an item's status.

```bash
giantt set-status SUBSTRING STATUS
```

Status options:
- NOT_STARTED
- IN_PROGRESS
- BLOCKED
- COMPLETED

Example:
```bash
giantt set-status python IN_PROGRESS
```

### sort
Sort items in topological order based on dependencies.

```bash
giantt sort [OPTIONS]
```

Options:
- `--file, -f`: Specify items file
- `--occlude-file, -a`: Specify occlude items file

### insert
Insert a new item between two existing items.

```bash
giantt insert NEW_ID BEFORE_ID AFTER_ID [OPTIONS]
```

Options:
- `--charts`: Comma-separated chart names
- `--tags`: Comma-separated tags
- `--duration`: Duration specification
- `--priority`: Priority level

Example:
```bash
giantt insert setup_env git_basics python_basics --duration 2d --priority MEDIUM
```

### occlude
Occlude items or logs that should no longer be included in decisionmaking with LLMs.

#### Occlude Items
```bash
giantt occlude items [IDENTIFIERS...] [OPTIONS]
```

Options:
- `-t, --tag`: Occlude items with specific tags
- `--dry-run`: Preview what would be occluded
- `--file, -f`: Specify items file
- `--occlude-file, -a`: Specify occlude items file

Example:
```bash
giantt occlude items -t completed_project --dry-run
giantt occlude items old_task1 old_task2
```

#### Occlude Logs
```bash
giantt occlude logs [IDENTIFIERS...] [OPTIONS]
```

Options:
- `-t, --tag`: Occlude logs with specific tags
- `--dry-run`: Preview what would be occluded
- `--file, -f`: Specify log file
- `--occlude-file, -a`: Specify occlude log file

### log
Create a log entry with session tag and message.

```bash
giantt log SESSION MESSAGE [OPTIONS]
```

Options:
- `--tags`: Additional comma-separated tags
- `--file, -f`: Specify log file
- `--occlude-file, -a`: Specify occlude log file

Example:
```bash
giantt log dev0 "Initial project setup" --tags setup,planning
```

### doctor
Check the health of the Giantt graph and optionally fix issues.

```bash
giantt doctor [OPTIONS]
```

Options:
- `--file, -f`: Specify items file
- `--occlude-file, -a`: Specify occlude items file
- `--fix/--no-fix`: Attempt to automatically fix detected issues

### touch
Touch items and logs files to trigger a reload and save. This command loads all files and saves them back, which can be useful to resolve minor issues and ensure a consistent state.

```bash
giantt touch [OPTIONS]
```

Options:
- `--file, -f`: Specify items file
- `--occlude-file, -a`: Specify occlude items file
- `--log-file, -l`: Specify log file
- `--occlude-log-file, -al`: Specify occlude log file

### clean
Clean up backup files, keeping only the most recent few backups.

```bash
giantt clean [OPTIONS]
```

Options:
- `--yes, -y`: Skip confirmation prompt
- `--keep, -k`: Number of recent backups to keep (default: 3)

## Status Symbols
- `○` NOT_STARTED
- `◑` IN_PROGRESS
- `⊘` BLOCKED
- `●` COMPLETED

## Priority Levels
- `,,,` LOWEST
- `...` LOW
- ` ` (none) NEUTRAL
- `?` UNSURE
- `!` MEDIUM
- `!!` HIGH
- `!!!` CRITICAL

## Relation Types
- `⊢` REQUIRES (must complete before)
- `⋲` ANYOF (any one is sufficient)
- `≫` SUPERCHARGES (optional enhancement)
- `∴` INDICATES (suggests consequence)
- `∪` TOGETHER (shows combination)
- `⊟` CONFLICTS (suggests blocking)
- `►` BLOCKS (shows items requiring this item)
- `≻` SUFFICIENT (shows items for which this is sufficient)