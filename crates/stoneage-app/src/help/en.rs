//! English user guide.

use super::Topic;

pub(super) const TOPICS: &[Topic] = &[
    Topic {
        id: "start",
        title: "Getting started",
        body: r#"
StoneAge records frequency soundings with a Velleman PCSGU250 USB oscilloscope and turns them into depth sections, depth slices (horizontal cuts), maps and a 3D block of the subsurface.

# The window
- Left: the sidebar with the sections Files, Measure, Survey, Display, Data, Help and Messages. Click an icon in the rail to switch, click the active icon again to collapse the panel.
- Top: the menu bar (all original menu items) and the view tabs Map, 3D, Areal, Row, Point, Soundtest and Soundtest 2.
- Bottom: the status bar with "Row x / y" and "Point x / y" including the << >> buttons, the oscilloscope state, read-outs of the value under the mouse and the "modified" indicator.

# Quick start
- Connect the oscilloscope and the probe, then open Measure and press "start Oszi".
- Open Survey, enter a title, the point distance and row distance and choose Kb and the frequency range f. Press Apply.
- Walk to the first point and press Record (or Space). Every Record stores the current point and adds a new, empty one.
- At the end of a row use "Rec + new row": the current point is recorded and a new row with one new point is started.
- Save the survey with Ctrl+S (*.saf).

> Without an oscilloscope StoneAge runs in demo mode and generates synthetic data, so every function can be tried out.
"#,
    },
    Topic {
        id: "workflow",
        title: "Survey workflow",
        body: r#"
A survey (file *.saf) consists of rows; every row consists of points. Points are recorded along a line at a constant point distance, rows lie parallel at a constant row distance. Together they form a regular grid that the Map, Areal and 3D views display.

# 1. Prepare
- Create a new survey (Files → New, Ctrl+N). The parameters are initialised from the defaults in StoneAge.ini.
- Survey section: enter Title, Date ("Now" inserts the current date and time) and a Description.
- Enter point distance and row distance in metres.
- Choose Kb (ground type) and the frequency range f. f must be identical to the frequency range set on the oscilloscope.
- Check delta (Δfh) and Tm. Press Apply.

# 2. Start the oscilloscope
- Measure section → "start Oszi". The status card turns green ("Connected").
- "show oszi" opens the vendor oscilloscope window to check the signal, volts/div and time base.

# 3. Record
- Place the probe on the first point and press Record (Space).
- Move one point distance further and press Record again – and so on.
- "Record delayed" (Ctrl+Space) waits for the delay configured in StoneAge.ini (RecordDelay, default 5 s) before recording – useful when you have to walk to the probe first. A countdown is shown; Cancel aborts it.
- At the end of a row: "Rec + new row" records the point and starts the next row.

# 4. Check
- Point view: spectrum of the point just recorded. A warning "Pointdata not valid – check cable and device…" means the data is unusable; record the point again.
- Row view: the section of the current row grows with every point.

# 5. Evaluate
- Areal and Map views show depth slices; move the horizontal cut slider in Display.
- Adjust zoom, block size, interpolation and colours in Display.
- Use find Min/Max to stretch the colour scale to the data of the row.

# 6. Save and export
- Save (Ctrl+S) or Save As (Ctrl+Shift+S).
- Export CSV for spreadsheets, Export image for reports, Print.
"#,
    },
    Topic {
        id: "parameters",
        title: "Parameters and formulas",
        body: r#"
Every point holds 2048 samples (index i = 0 … 2047) – the spectrum read from the oscilloscope. The vertical axis of the Row, Areal and 3D views is this sample index, shown either as frequency or as calculated depth.

# Kb – ground constant
Depends on the ground at the survey site. The list offers ground types -1 … 13; the number in brackets is Kb × 1000 (e.g. "Ground # 6 (1100)" = Kb 1.1). Choose "Custom" to type any value (× 1000). Kb is proportional to the calculated depth.

# f – frequency range [kHz]
The frequency range of the oscilloscope: 30, 60, 120, 300 kHz or 0.6, 1.2, 3, 6 MHz. It must be the same as on the oscilloscope, otherwise all depths are wrong. Each recorded point additionally stores the range that was active when it was recorded.

# delta – Δfh
Frequency difference constant of the method (default 400). The depth is inversely proportional to Δfh.

# Tm [s]
Time constant of the measurement (default 0.02 s). Stored in the survey file; the depth is proportional to Tm.

# Depth of sample i
Exactly as calculated by the original (MessReihe.tiefe):
$ f(i) = 500 · fRange / 2048 · i
$ t(i) = 100 · Kb · Tm · f(i) / (2 · Δfh)
The result is rounded to centimetres. In short:
$ t(i) = 25000 · Kb · Tm · fRange · i / (2048 · Δfh)   [m]
Example with the defaults Kb = 1.1, Tm = 0.02, fRange = 300, Δfh = 400: 0.20 m per sample, 103.13 m at sample 512 and 412.5 m at sample 2048.

# Frequency labels
The frequency axis (MessReihe.freq) uses the range stored in the point and the display settings:
$ f = round(0.1 · BlockSize · (y / ZoomY) · fRange / 2048) / 100   [MHz]
where y is the pixel position. The frequency step of one sample is fRange / 2048.

# Point and row distance [m]
Distance between neighbouring points of a row and between neighbouring rows. They define the x and z positions in the Map, Areal and 3D views.

# Apply
Text fields take effect when you press Enter in the field or press Apply. Combo boxes, check boxes and sliders take effect immediately – and like in the original every change applies all fields at once. Invalid numbers are rejected, the old value is restored and a message is written to Messages.
"#,
    },
    Topic {
        id: "recording",
        title: "Recording",
        body: r#"
# Buttons
| Record | records the current point and inserts a new one (Space) |
| Rec + new row | records the current point and creates a new row with one new point (Shift+Space) |
| Record delayed | like Record, after the record delay (Ctrl+Space) |
| Delayed Record + new row | like Rec + new row, after the record delay |

The last point of the current row is always the "open" point that the next Record fills. If it has already been recorded, a new point is created and recorded.

# Delay
The delay is RecordDelay in StoneAge.ini (milliseconds, default 5000). While the countdown runs a progress bar is shown; Cancel aborts the pending recording.

# Data check
After recording the data is checked (MessPunkt.isDataValid). If it is not valid the message "Pointdata not valid – check cable and device…" appears in the Measure section and in Messages, and the point is marked with ⚠ in Data.

# Oscilloscope
- start Oszi / stop Oszi starts and stops data acquisition of the PCSGU250.
- show oszi shows or hides the vendor's oscilloscope window (volts/div, time base, trigger).
- In demo mode synthetic but realistic data is produced.
"#,
    },
    Topic {
        id: "views",
        title: "Views",
        body: r#"
Switch views with the tabs at the top or Ctrl+1 … Ctrl+7.

# Map (Ctrl+1)
Top view of the survey area. Every point is drawn at its position (point distance × row distance), coloured by the value at the depth of the horizontal cut. A background map (Files → Load map, JPG/GIF/PNG) can be placed under the grid – position, scale and rotation are stored in the survey.

# 3D (Ctrl+2)
The whole survey as a three-dimensional block: x = points, y = depth/frequency, z = rows. Drag with the mouse to rotate, use the wheel to zoom. Zoom X/Y/Z in Display scale the axes.

# Areal (Ctrl+3)
All rows side by side as a depth slice at the horizontal cut ("Depth-cut at … m"). Useful to recognise structures that run across several rows.

# Row (Ctrl+4)
Vertical section of the current row: x = position of the point in metres, y = frequency or (with "calculate depth") depth, colour = amplitude. The horizontal line marks the horizontal cut. Hover to read value, frequency/depth and position.

# Point (Ctrl+5)
Spectrum (or signal) of the current point with its source (spectrum/signal), frequency range and signal length. Use it to check every recording.

# Soundtest / Soundtest 2 (Ctrl+6, Ctrl+7)
Capture the signal from the PC sound input. Useful to check probe and cable without the oscilloscope.

# Colour legend
The scale bar shows the colour model; drag it to shift the colour base. Display → Colours selects the model and reverses it.
"#,
    },
    Topic {
        id: "display",
        title: "Display settings",
        body: r#"
- Zoom X – width of one point on screen.
- Zoom Y – height of one sample.
- Zoom Z – width of one row (3D/Areal).
- Block size – number of samples combined into one block (1 … 32).
- use block-average – average the samples of a block instead of taking single samples.
- Interpolate – smooth colour transitions between points.
- Show raster – draw the measurement grid.
- calculate depth – label the vertical axis with the calculated depth instead of the frequency.
- Horizontal cut – sample index (0 … 2048) of the depth slice shown in Map/Areal; the labels below the slider show the depth in metres.
- Valid base / valid step – only shown when showValidBase = true in StoneAge.ini: which points of the row are treated as valid (first valid point and distance between valid points).
- Colour model – choose a palette; Reverse colours inverts it.

All display settings are stored in the survey file.
"#,
    },
    Topic {
        id: "data",
        title: "Editing data",
        body: r#"
The Data section lists all rows with their number of points and the points of the current row with recording time and status (✔ valid, ⚠ not valid, ○ not yet recorded). Click an entry to make it current.

- Insert new Row – inserts an empty row after the current one.
- Delete current row / Delete current Point.
- Delete all rows – asks for confirmation; cannot be undone (except by not saving).
- find Min/Max – determines minimum and maximum of the current row and fits the colour scale to it.
- reset Min/Max – back to the full range.

Navigation: << >> in the status bar, the arrow keys (← → point, ↑ ↓ row) or Edit → Previous/Next Row.
"#,
    },
    Topic {
        id: "files",
        title: "Files and formats",
        body: r#"
# StoneAge files (*.saf)
The survey with all parameters, display settings, map placement and all rows and points. Format identifier: {format}. Files of the original StoneAge can be opened and are written in the same format.

# CSV
Export CSV writes the measured values as a table for spreadsheets; Import CSV reads such a table back.

# Map images
Load map accepts JPG, GIF and PNG. The image itself is not stored in the .saf file – only its placement.

# Export image / Print
Export the current view as an image file or print it.

# StoneAge.ini – defaults
Language, Country, RecordDelay, PointDistance, RowDistance, dfh, Kb, fRange, disp_stepx/y/z, colorModel, colorModelBase and showValidBase. "Save Defaults" writes the parameters of the current survey to this file; new surveys start with them.

# Recent files
The Files section lists recently used files; click one to open it. Files that no longer exist are struck through.
"#,
    },
    Topic {
        id: "shortcuts",
        title: "Keyboard shortcuts",
        body: r#"
| Ctrl+N | New survey |
| Ctrl+O | Open |
| Ctrl+S | Save |
| Ctrl+Shift+S | Save as |
| Ctrl+P | Print |
| Ctrl+Q | Exit |
| Space | Record |
| Ctrl+Space | Record delayed |
| Shift+Space | Rec + new row |
| Esc | Cancel the delayed recording |
| ← / → | Previous / next point |
| ↑ / ↓ | Previous / next row |
| T | Toggle tracer (messages) |
| F1 | Help |
| Ctrl+1 … Ctrl+7 | Map, 3D, Areal, Row, Point, Soundtest, Soundtest 2 |
| Enter | Apply the text field (Survey) |

> Space and the arrow keys are ignored while a text field has the keyboard focus.
"#,
    },
    Topic {
        id: "troubleshooting",
        title: "Troubleshooting",
        body: r#"
# Status "Demo" although the oscilloscope is connected
- PCSGU250.dll could not be loaded. The reason is written to Messages.
- The Velleman DLL is 32-bit: the real oscilloscope only works with the 32-bit (x86) build of StoneAge.
- PCSGU250.dll must be in the program directory (or its bin folder) or on the PATH.

# Status "not ready"
- Check the USB cable; connect directly to the PC, not via a hub; try another port.
- Install or reinstall the Velleman PCSGU250 USB driver (Device Manager must show the device without warning sign).
- Close other programs that use the oscilloscope (e.g. PcLab2000) – only one program can access it.
- Press "stop Oszi" and "start Oszi" again, or unplug and reconnect the device.

# "Pointdata not valid – check cable and device…"
- Check probe, cable and ground connection.
- Make sure the oscilloscope is running (start Oszi) and the signal is visible (show oszi).
- The frequency range f must match the oscilloscope setting.
- Record the point again.

# Depths look wrong
- Check Kb, Δfh, Tm and the frequency range f; see "Parameters and formulas".

# Where are messages?
The Messages section shows the program log. Enable "Debug" for detailed output and use "Copy all" when you report a problem.
"#,
    },
    Topic {
        id: "about",
        title: "About StoneAge",
        body: r#"
StoneAge 2 – version {app_version}.
Independent re-implementation of StoneAge {version} with a modern user interface. Not affiliated with or endorsed by the maker of the original software.

- Original version: {version}
- File format (.saf): {format}
- Oscilloscope: Velleman PCSGU250 (USB), via PCSGU250.dll
- Languages: German, English, Turkish

StoneAge 2 (c) 2026 Tamer Mansour.
"#,
    },
    Topic {
        id: "license",
        title: "License",
        body: "",
    },
];
