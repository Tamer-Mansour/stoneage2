//! Deutsches Benutzerhandbuch.

use super::Topic;

pub(super) const TOPICS: &[Topic] = &[
    Topic {
        id: "start",
        title: "Erste Schritte",
        body: r#"
StoneAge zeichnet Frequenzsondierungen mit dem USB-Oszilloskop Velleman PCSGU250 auf und erzeugt daraus Tiefenschnitte, Horizontalschnitte, Karten und einen 3D-Block des Untergrunds.

# Das Fenster
- Links: die Seitenleiste mit den Bereichen Dateien, Messen, Messung, Anzeige, Daten, Hilfe und Meldungen. Ein Klick auf ein Symbol wechselt den Bereich, ein erneuter Klick auf das aktive Symbol klappt die Leiste ein.
- Oben: die Menüleiste (alle Menüpunkte des Originals) und die Ansichten Karte, 3D, Areal, Reihe, Punkt, Soundtest und Soundtest 2.
- Unten: die Statusleiste mit „Reihe x / y“ und „Punkt x / y“ samt << >>-Tasten, dem Oszilloskop-Zustand, Messwerten unter dem Mauszeiger und der Anzeige „geändert“.

# Schnellstart
- Oszilloskop und Sonde anschließen, im Bereich Messen „start Oszi“ drücken.
- Im Bereich Messung Titel, Punktabstand und Reihenabstand eingeben, Kb und den Frequenzbereich f wählen und „Übernehmen“ drücken.
- Zum ersten Punkt gehen und Aufnahme (oder Leertaste) drücken. Jede Aufnahme speichert den aktuellen Punkt und legt einen neuen, leeren Punkt an.
- Am Ende einer Reihe „Aufn. + neue Reihe“ verwenden: der Punkt wird aufgenommen und eine neue Reihe mit einem neuen Punkt begonnen.
- Messung mit Strg+S speichern (*.saf).

> Ohne Oszilloskop läuft StoneAge im Demo-Modus und erzeugt synthetische Daten – so lassen sich alle Funktionen ausprobieren.
"#,
    },
    Topic {
        id: "workflow",
        title: "Ablauf einer Messung",
        body: r#"
Eine Messung (Datei *.saf) besteht aus Reihen, jede Reihe aus Punkten. Die Punkte werden entlang einer Linie im festen Punktabstand aufgenommen, die Reihen liegen parallel im festen Reihenabstand. Zusammen ergibt sich ein regelmäßiges Raster, das die Ansichten Karte, Areal und 3D darstellen.

# 1. Vorbereiten
- Neue Messung anlegen (Dateien → Neu, Strg+N). Die Parameter kommen aus den Grundeinstellungen in StoneAge.ini.
- Bereich Messung: Titel, Datum („Jetzt“ setzt aktuelles Datum und Uhrzeit) und Bemerkung eingeben.
- Punktabstand und Reihenabstand in Metern eingeben.
- Kb (Bodenart) und den Frequenzbereich f wählen. f muss mit dem am Oszilloskop eingestellten Bereich übereinstimmen.
- delta (Δfh) und Tm prüfen. „Übernehmen“ drücken.

# 2. Oszilloskop starten
- Bereich Messen → „start Oszi“. Die Statuskarte wird grün („Verbunden“).
- „zeige Oszi“ öffnet das Oszilloskop-Fenster des Herstellers, um Signal, Volt/Div und Zeitbasis zu prüfen.

# 3. Aufnehmen
- Sonde auf den ersten Punkt setzen und Aufnahme (Leertaste) drücken.
- Einen Punktabstand weitergehen, erneut Aufnahme drücken – und so fort.
- „Verzögerte Aufnahme“ (Strg+Leertaste) wartet die in StoneAge.ini eingestellte Verzögerung ab (RecordDelay, Standard 5 s) – nützlich, wenn man erst zur Sonde gehen muss. Ein Countdown wird angezeigt, „Abbrechen“ bricht ab.
- Am Ende einer Reihe: „Aufn. + neue Reihe“ nimmt den Punkt auf und beginnt die nächste Reihe.

# 4. Kontrollieren
- Ansicht Punkt: Spektrum des gerade aufgenommenen Punktes. Die Warnung „Punktdaten ungültig …“ bedeutet unbrauchbare Daten – Punkt neu aufnehmen.
- Ansicht Reihe: der Schnitt der aktuellen Reihe wächst mit jedem Punkt.

# 5. Auswerten
- Areal und Karte zeigen Horizontalschnitte; dazu den Regler „Horizontalschnitt“ im Bereich Anzeige verschieben.
- Zoom, Blockgröße, Interpolation und Farben im Bereich Anzeige anpassen.
- „finde Min/Max“ passt die Farbskala an die Daten der Reihe an.

# 6. Speichern und exportieren
- Speichern (Strg+S) oder Speichern unter (Strg+Umschalt+S).
- Export CSV für Tabellenkalkulationen, Bild exportieren für Berichte, Drucken.
"#,
    },
    Topic {
        id: "parameters",
        title: "Parameter und Formeln",
        body: r#"
Jeder Punkt enthält 2048 Messsamples (Index i = 0 … 2047) – das vom Oszilloskop gelesene Spektrum. Die senkrechte Achse der Ansichten Reihe, Areal und 3D ist dieser Index, dargestellt als Frequenz oder als berechnete Tiefe.

# Kb – Bodenkonstante
Hängt vom Boden am Messort ab. Die Liste bietet die Bodenarten -1 … 13; die Zahl in Klammern ist Kb × 1000 (z. B. „Bodenart 6 (1100)“ = Kb 1,1). Mit „Benutzerdefiniert“ kann ein beliebiger Wert (× 1000) eingegeben werden. Die berechnete Tiefe ist proportional zu Kb.

# f – Frequenzbereich [kHz]
Der Frequenzbereich des Oszilloskops: 30, 60, 120, 300 kHz oder 0,6, 1,2, 3, 6 MHz. Er muss mit dem Oszilloskop übereinstimmen, sonst sind alle Tiefen falsch. Jeder Punkt speichert zusätzlich den Bereich, mit dem er aufgenommen wurde.

# delta – Δfh
Frequenzdifferenz-Konstante des Verfahrens (Standard 400). Die Tiefe ist umgekehrt proportional zu Δfh.

# Tm [s]
Zeitkonstante der Messung (Standard 0,02 s). Wird in der Messungsdatei gespeichert; die Tiefe ist proportional zu Tm.

# Tiefe des Samples i
Genau wie im Original berechnet (MessReihe.tiefe):
$ f(i) = 500 · fRange / 2048 · i
$ t(i) = 100 · Kb · Tm · f(i) / (2 · Δfh)
Das Ergebnis wird auf Zentimeter gerundet. Zusammengefasst:
$ t(i) = 25000 · Kb · Tm · fRange · i / (2048 · Δfh)   [m]
Beispiel mit den Standardwerten Kb = 1,1, Tm = 0,02, fRange = 300, Δfh = 400: 0,20 m pro Sample, 103,13 m bei Sample 512 und 412,5 m bei Sample 2048.

# Frequenzbeschriftung
Die Frequenzachse (MessReihe.freq) verwendet den im Punkt gespeicherten Bereich und die Anzeigeeinstellungen:
$ f = round(0,1 · BlockSize · (y / ZoomY) · fRange / 2048) / 100   [MHz]
y ist die Pixelposition. Der Frequenzschritt eines Samples beträgt fRange / 2048.

# Punkt- und Reihenabstand [m]
Abstand benachbarter Punkte einer Reihe und benachbarter Reihen. Sie bestimmen die x- und z-Positionen in Karte, Areal und 3D.

# Übernehmen
Textfelder werden mit der Eingabetaste im Feld oder mit „Übernehmen“ wirksam. Auswahllisten, Kontrollkästchen und Regler wirken sofort – und wie im Original übernimmt jede Änderung alle Felder auf einmal. Ungültige Zahlen werden verworfen, der alte Wert wiederhergestellt und eine Meldung ausgegeben.
"#,
    },
    Topic {
        id: "recording",
        title: "Aufnahme",
        body: r#"
# Schaltflächen
| Aufnahme | nimmt den aktuellen Punkt auf und fügt einen neuen ein (Leertaste) |
| Aufn. + neue Reihe | nimmt den aktuellen Punkt auf und legt eine neue Reihe mit einem neuen Punkt an (Umschalt+Leertaste) |
| Verzögerte Aufnahme | wie Aufnahme, nach der Aufnahmeverzögerung (Strg+Leertaste) |
| Verzögerte Aufnahme + neue Reihe | wie Aufn. + neue Reihe, nach der Aufnahmeverzögerung |

Der letzte Punkt der aktuellen Reihe ist immer der „offene“ Punkt, den die nächste Aufnahme füllt. Ist er bereits aufgenommen, wird ein neuer Punkt angelegt und aufgenommen.

# Verzögerung
Die Verzögerung ist RecordDelay in StoneAge.ini (Millisekunden, Standard 5000). Während des Countdowns wird ein Fortschrittsbalken angezeigt; „Abbrechen“ verwirft die Aufnahme.

# Datenprüfung
Nach der Aufnahme werden die Daten geprüft (MessPunkt.isDataValid). Sind sie ungültig, erscheint „Punktdaten ungültig – Kabel und Gerät prüfen …“ im Bereich Messen und in den Meldungen, und der Punkt wird im Bereich Daten mit ⚠ markiert.

# Oszilloskop
- start Oszi / stop Oszi startet und stoppt die Datenerfassung des PCSGU250.
- zeige Oszi blendet das Oszilloskop-Fenster des Herstellers ein oder aus (Volt/Div, Zeitbasis, Trigger).
- Im Demo-Modus werden synthetische, aber realistische Daten erzeugt.
"#,
    },
    Topic {
        id: "views",
        title: "Ansichten",
        body: r#"
Ansichten über die Reiter oben oder mit Strg+1 … Strg+7 wechseln.

# Karte (Strg+1)
Draufsicht auf das Messgebiet. Jeder Punkt wird an seiner Position (Punktabstand × Reihenabstand) gezeichnet, eingefärbt nach dem Wert in der Tiefe des Horizontalschnitts. Unter das Raster kann eine Karte gelegt werden (Dateien → Lade Karte, JPG/GIF/PNG); Position, Maßstab und Drehung werden in der Messung gespeichert.

# 3D (Strg+2)
Die ganze Messung als dreidimensionaler Block: x = Punkte, y = Tiefe/Frequenz, z = Reihen. Mit der Maus ziehen zum Drehen, Mausrad zum Zoomen. Zoom X/Y/Z im Bereich Anzeige skalieren die Achsen.

# Areal (Strg+3)
Alle Reihen nebeneinander als Horizontalschnitt („Tiefenschnitt bei … m“). Gut geeignet, um Strukturen über mehrere Reihen zu erkennen.

# Reihe (Strg+4)
Senkrechter Schnitt der aktuellen Reihe: x = Position des Punktes in Metern, y = Frequenz oder (mit „Tiefe berechnen“) Tiefe, Farbe = Amplitude. Die waagrechte Linie markiert den Horizontalschnitt. Der Mauszeiger zeigt Wert, Frequenz/Tiefe und Position.

# Punkt (Strg+5)
Spektrum (oder Signal) des aktuellen Punktes mit Quelle (Spektrum/Signal), Frequenzbereich und Signallänge. Zur Kontrolle jeder Aufnahme.

# Soundtest / Soundtest 2 (Strg+6, Strg+7)
Erfassen das Signal über den Soundeingang des PCs – zum Prüfen von Sonde und Kabel ohne Oszilloskop.

# Farblegende
Die Skala zeigt das Farbmodell; durch Ziehen wird die Farbbasis verschoben. Anzeige → Farben wählt das Modell und kehrt es um.
"#,
    },
    Topic {
        id: "display",
        title: "Anzeigeeinstellungen",
        body: r#"
- Zoom X – Breite eines Punktes auf dem Bildschirm.
- Zoom Y – Höhe eines Messsamples.
- Zoom Z – Breite einer Reihe (3D/Areal).
- Blockgröße – Anzahl der zusammengefassten Messsamples (1 … 32).
- Block-Mittelwert – die Samples eines Blocks mitteln statt Einzelwerte zu verwenden.
- Interpolation – weiche Farbübergänge zwischen den Punkten.
- Raster anzeigen – das Messraster zeichnen.
- Tiefe berechnen – senkrechte Achse mit der berechneten Tiefe statt der Frequenz beschriften.
- Horizontalschnitt – Sample-Index (0 … 2048) des Tiefenschnitts in Karte/Areal; die Beschriftung unter dem Regler zeigt die Tiefe in Metern.
- Gültige Basis / Schritt – nur sichtbar mit showValidBase = true in StoneAge.ini: welche Punkte der Reihe als gültig gelten (erster gültiger Punkt und Abstand zwischen gültigen Punkten).
- Farbmodell – Palette wählen; „Farben umkehren“ invertiert sie.

Alle Anzeigeeinstellungen werden in der Messungsdatei gespeichert.
"#,
    },
    Topic {
        id: "data",
        title: "Daten bearbeiten",
        body: r#"
Der Bereich Daten listet alle Reihen mit ihrer Punktanzahl und die Punkte der aktuellen Reihe mit Aufnahmezeit und Status (✔ gültig, ⚠ ungültig, ○ noch nicht aufgenommen). Ein Klick macht den Eintrag aktuell.

- Neue Reihe einfügen – fügt nach der aktuellen Reihe eine leere Reihe ein.
- Aktuelle Reihe löschen / Aktuellen Punkt löschen.
- Alle Reihen löschen – mit Rückfrage; kann nicht rückgängig gemacht werden (außer durch Nicht-Speichern).
- finde Min/Max – bestimmt Minimum und Maximum der aktuellen Reihe und passt die Farbskala an.
- reset Min/Max – zurück auf den vollen Bereich.

Navigation: << >> in der Statusleiste, Pfeiltasten (← → Punkt, ↑ ↓ Reihe) oder Bearbeiten → Vorherige/Nächste Reihe.
"#,
    },
    Topic {
        id: "files",
        title: "Dateien und Formate",
        body: r#"
# StoneAge-Dateien (*.saf)
Die Messung mit allen Parametern, Anzeigeeinstellungen, Kartenplatzierung und allen Reihen und Punkten. Formatkennung: {format}. Dateien des Original-StoneAge können geöffnet werden und werden im selben Format geschrieben.

# CSV
Export CSV schreibt die Messwerte als Tabelle für Tabellenkalkulationen; Import CSV liest eine solche Tabelle wieder ein.

# Kartenbilder
Lade Karte akzeptiert JPG, GIF und PNG. Das Bild selbst wird nicht in der .saf-Datei gespeichert – nur seine Platzierung.

# Bild exportieren / Drucken
Die aktuelle Ansicht als Bilddatei exportieren oder drucken.

# StoneAge.ini – Grundeinstellungen
Language, Country, RecordDelay, PointDistance, RowDistance, dfh, Kb, fRange, disp_stepx/y/z, colorModel, colorModelBase und showValidBase. „Grundeinstellungen speichern“ schreibt die Parameter der aktuellen Messung in diese Datei; neue Messungen beginnen damit.

# Zuletzt verwendet
Der Bereich Dateien listet die zuletzt verwendeten Dateien; ein Klick öffnet sie. Nicht mehr vorhandene Dateien sind durchgestrichen.
"#,
    },
    Topic {
        id: "shortcuts",
        title: "Tastenkürzel",
        body: r#"
| Strg+N | Neue Messung |
| Strg+O | Öffnen |
| Strg+S | Speichern |
| Strg+Umschalt+S | Speichern unter |
| Strg+P | Drucken |
| Strg+Q | Beenden |
| Leertaste | Aufnahme |
| Strg+Leertaste | Verzögerte Aufnahme |
| Umschalt+Leertaste | Aufn. + neue Reihe |
| Esc | Verzögerte Aufnahme abbrechen |
| ← / → | Vorheriger / nächster Punkt |
| ↑ / ↓ | Vorherige / nächste Reihe |
| T | Protokoll ein/aus |
| F1 | Hilfe |
| Strg+1 … Strg+7 | Karte, 3D, Areal, Reihe, Punkt, Soundtest, Soundtest 2 |
| Eingabe | Textfeld übernehmen (Messung) |

> Leertaste und Pfeiltasten werden ignoriert, solange ein Textfeld den Tastaturfokus hat.
"#,
    },
    Topic {
        id: "troubleshooting",
        title: "Fehlerbehebung",
        body: r#"
# Status „Demo“, obwohl das Oszilloskop angeschlossen ist
- PCSGU250.dll konnte nicht geladen werden. Der Grund steht in den Meldungen.
- Die Velleman-DLL ist 32-bit: das echte Oszilloskop funktioniert nur mit der 32-bit-Version (x86) von StoneAge.
- PCSGU250.dll muss im Programmverzeichnis (oder dessen bin-Ordner) oder im PATH liegen.

# Status „nicht bereit“
- USB-Kabel prüfen; direkt am PC anschließen, nicht über einen Hub; anderen Anschluss probieren.
- Velleman-PCSGU250-USB-Treiber (neu) installieren – im Geräte-Manager muss das Gerät ohne Warnsymbol erscheinen.
- Andere Programme schließen, die das Oszilloskop verwenden (z. B. PcLab2000) – nur ein Programm kann darauf zugreifen.
- „stop Oszi“ und erneut „start Oszi“ drücken oder das Gerät aus- und wieder einstecken.

# „Punktdaten ungültig – Kabel und Gerät prüfen …“
- Sonde, Kabel und Masseverbindung prüfen.
- Sicherstellen, dass das Oszilloskop läuft (start Oszi) und das Signal sichtbar ist (zeige Oszi).
- Der Frequenzbereich f muss zur Oszilloskop-Einstellung passen.
- Punkt erneut aufnehmen.

# Tiefen erscheinen falsch
- Kb, Δfh, Tm und den Frequenzbereich f prüfen; siehe „Parameter und Formeln“.

# Wo stehen Meldungen?
Der Bereich Meldungen zeigt das Programmprotokoll. „Debug“ aktiviert ausführliche Ausgaben; mit „Alles kopieren“ lässt sich das Protokoll für eine Fehlermeldung weitergeben.
"#,
    },
    Topic {
        id: "about",
        title: "Über StoneAge",
        body: r#"
StoneAge 2 – Version {app_version}.
Unabhängige Neuimplementierung von StoneAge {version} mit moderner Oberfläche. Keine Verbindung zum Hersteller der ursprünglichen Software.

- Originalversion: {version}
- Dateiformat (.saf): {format}
- Oszilloskop: Velleman PCSGU250 (USB), über PCSGU250.dll
- Sprachen: Deutsch, Englisch, Türkisch

StoneAge 2 (c) 2026 Tamer Mansour.
"#,
    },
    Topic {
        id: "license",
        title: "Lizenz",
        body: "",
    },
];
