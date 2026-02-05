# Giantt Item Notation System

Each item is represented on a single line with the following format:  
`status id+priority duration title {charts} [tags] [>>> relations] [@@@ time_constraints] [# comment] [### auto_comment]`

## Basic Structure

### Status
#### Status symbols:
- `○` NOT_STARTED
- `◑` IN_PROGRESS
- `⊘` BLOCKED
- `●` COMPLETED

### Item Identifier and priority
#### Identifiers
- Simple alphanumeric ID with underscores
- Example: `paint_house`, `learn_python`

#### Priority symbols:
- `,,,` LOWEST PRIORITY
- `...` LOW
- ` ` (none) NEUTRAL
- `?` UNSURE IF NEEDED
- `!` MEDIUM
- `!!` HIGH
- `!!!` CRITICAL

#### Format
These are concatenated with no space in between.

Examples
- `learn_python!!`
- `house_paint!`
- `learn_guitar,,,`

### Duration:
- Number followed by unit abbreviation (s/min/hr/d/w/mo/y)
- Can be strung together
- Example: `14d`, `6mo`, `1y`, `18mo2w15hr2s`

### Title:
- Required
- JSON-formatted string: starts and ends with double quotes, and may include any JSON-valid escape sequences

### Charts Block `{charts}`
- Comma-separated list of chart names written as full double-quoted strings wrapped in curly braces
- Examples:
  - `{"Home Improvement"}`
  - `{"Programming","Education"}`

### Tags Block
- Optional comma-separated list of lowercase-only alphanumeric tags plus underscores, no brackets
- Examples:
  - `person_lily`
  - `rim,cryptography,person_jaguar`

### Relations
Each relation type uses a symbol followed by comma-separated target IDs:
- `⊢` REQUIRES (left side tick)
- `⋲` ANY (alternate paths)
- `≫` SUPERCHARGES (much greater than - suggests enhancement)
- `∴` INDICATES (therefore - suggests consequence)
- `∪` WITH (shows combination)
- `⊟` CONFLICTS (suggests blocking)
- `►` (redundancy symbol) BLOCKS (shows all items with ⊢ [REQUIRES] to this item)
- `≻` (redundancy symbol) SUFFICIENT (shows all items with ⋲ [ANY] to this item)

Formatting
- Square brackets surround the relation targets
- Multiple targets use commas without spaces
- Example: `⊢[oop_online_course] ►[django_proj,flask_app,data_science]`

### Time Constraints
Time constraints are added after the relations block using `@@@`:

#### Window Constraints
For tasks that must be completed within a time window:
```
@@@ window(5d,severe)      # Must complete within 5 days, severe consequence
@@@ window(3d:2d,warn)     # 3 day window with 2 day grace period, warning only
@@@ window(7d,escalate:!!) # 7 days, rapidly escalating consequences
```

#### Deadline Constraints
For tasks with specific deadlines:
```
@@@ due(2025-03-01)           # Must complete by March 1st
@@@ due(2025-03-01,warn)      # Soft deadline with warning
@@@ due(2025-03-01:2d,severe) # Hard deadline with 2-day grace period
```

#### Recurring Constraints
For recurring tasks:
```
@@@ every(2w)                   # Recur every 2 weeks
@@@ every(1w:2d,warn)          # Weekly with 2-day window, warning only
@@@ every(3d,stack)            # Every 3 days, missed instances stack
@@@ every(1mo:3d,escalate:,,,) # Monthly with 3-day window, escalating only very slowly
```

### Comments
- Single hash (`#`) adds a persisted comment to the item
- Triple hash (`###`) adds a non-persisted auto-generated comment
- Lines beginning with hash are ignored entirely

## Complete Examples

```
○ learn_python!! 3mo "Finally learn python" {"Programming","Education"} personal_development >>> ⊢[git_basics] ►[django_proj,flask_app] ≫[web_dev] @@@ window(3mo,warn) # Start after finals ### Last modified: 2025-02-13 13:43:05

○ house_paint! 5d "Paint the house purple, again" {"Home Improvement"} 71_bonair,person_aubrey,9ants >>> ⊢[buy_paint,prep_walls] ↶[furniture_move] ⊗[floor_refinish] @@@ due(2025-04-15:2d,severe) # Weather dependent

⊘ write_thesis!!! 6mo "Thesis for MIT PhD" {"PhD","Research"} >>> ⊢[data_analysis] ↶[defense] ∪[lit_review] @@@ due(2025-06-01,severe) # Committee deadline

◑ water_plants! 15min "Plants just have to be watered every week, they're pretty hardy" {"Home"} housekeeping >>> @@@ every(7d:1d,escalate:...) # Including new succulents

# This is an ignored comment line
○ plan_vacation 2w "Vacation to London, Lily will book an AirBnb" {"Personal","Travel"} person_lily,london,person_tauntaun >>> ⊢[budget_check] ►[flight_booking] ∴[hotel_research] @@@ window(1mo,warn) # Before peak season

◑ startup_mvp!! 45d "Build MVP for rim--protocol at a minimum, hardware ideally" {"Business","Tech"} rim >>> ⊢[market_research] ►[beta_release] ∪[ui_design] ⊗[day_job]

○ learn_guitar,,, 1y "Be able to play more than just the top four strings like a uke..." {"Hobbies","Music"} personal_development >>> ►[basic_songs] ≫[composition] ∴[music_theory] ∪[daily_practice]
```

## File Header Banner
The GIANTT_ITEMS.txt file should begin with a banner of hash symbols containing metadata about the file itself. Example contents might include:

- File version/schema version
- Last modified timestamp and device
- Sync status
- Warning about auto-generated content

### Example format
- May change and may have many more items in the future, e.g. repos and other links, contributing, etc.
- Should not be considered to be consistent among or between versions of Giantt

```
##############################################################################
#                                GIANTT_ITEMS                                #
#      content following ### is auto-generated. EDITS WILL NOT PERSIST.      #
#              Last sync: device_id @ time Version: version                  #
##############################################################################
```