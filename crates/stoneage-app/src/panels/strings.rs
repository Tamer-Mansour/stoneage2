//! Built-in texts for the new `App.*` keys used by the sidebar panels / help center.
//! Used only while the keys are missing from `assets/i18n/*.properties` (core-misc adds them,
//! see docs/NOTES-app-panels.md). Columns: key, English, German, Turkish.

use stoneage_core::Language;

const TABLE: &[(&str, &str, &str, &str)] = &[
    // sections
    ("App.Section.Files", "Files", "Dateien", "Dosyalar"),
    ("App.Section.Measure", "Measure", "Messen", "Kayıt"),
    ("App.Section.Survey", "Survey", "Messung", "Ölçüm"),
    ("App.Section.Display", "Display", "Anzeige", "Görünüm"),
    ("App.Section.Data", "Data", "Daten", "Veri"),
    ("App.Section.Help", "Help", "Hilfe", "Yardım"),
    ("App.Section.Messages", "Messages", "Meldungen", "Mesajlar"),
    // common
    ("App.Common.Yes", "yes", "ja", "evet"),
    ("App.Common.No", "no", "nein", "hayır"),
    ("App.Common.Cancel", "Cancel", "Abbrechen", "İptal"),
    ("App.Common.Search", "Search", "Suchen", "Ara"),
    ("App.Common.Clear", "Clear", "Leeren", "Temizle"),
    ("App.Key.Space", "Space", "Leertaste", "Boşluk"),
    // files
    ("App.Files.Current", "Current file", "Aktuelle Datei", "Geçerli dosya"),
    ("App.Files.Untitled", "Untitled", "Unbenannt", "Adsız"),
    ("App.Files.Modified", "Modified", "Geändert", "Değiştirildi"),
    ("App.Files.Rows", "Rows", "Reihen", "Sıralar"),
    ("App.Files.Points", "Points", "Punkte", "Noktalar"),
    ("App.Files.Map", "Map", "Karte", "Harita"),
    ("App.Files.MapLoaded", "loaded", "geladen", "yüklendi"),
    ("App.Files.NoMap", "No map loaded", "Keine Karte geladen", "Harita yüklenmedi"),
    ("App.Files.MapSize", "Map image {0} × {1} px", "Kartenbild {0} × {1} px", "Harita görüntüsü {0} × {1} px"),
    ("App.Files.ClearMap", "Clear map", "Karte entfernen", "Haritayı kaldır"),
    ("App.Files.Document", "Document", "Dokument", "Belge"),
    ("App.Files.New", "New", "Neu", "Yeni"),
    ("App.Files.SaveAs", "Save as…", "Speichern unter…", "Farklı kaydet…"),
    ("App.Files.Recent", "Recent files", "Zuletzt verwendet", "Son kullanılanlar"),
    ("App.Files.NoRecent", "No recent files", "Keine zuletzt verwendeten Dateien", "Son kullanılan dosya yok"),
    ("App.Files.Missing", "File not found", "Datei nicht gefunden", "Dosya bulunamadı"),
    ("App.Files.ImportExport", "Import / Export", "Import / Export", "İçe / Dışa aktar"),
    ("App.Files.ExportImage", "Export image", "Bild exportieren", "Görüntü dışa aktar"),
    (
        "App.Files.ExportImage.ToolTip",
        "Save the current view as an image file",
        "Die aktuelle Ansicht als Bilddatei speichern",
        "Geçerli görünümü görüntü dosyası olarak kaydet",
    ),
    ("App.Files.Defaults", "Defaults", "Grundeinstellungen", "Varsayılanlar"),
    (
        "App.Files.SaveDefaults.Hint",
        "Stores the parameters of the current survey in StoneAge.ini; new surveys start with them.",
        "Speichert die Parameter der aktuellen Messung in StoneAge.ini; neue Messungen beginnen damit.",
        "Geçerli ölçümün parametrelerini StoneAge.ini dosyasına yazar; yeni ölçümler bunlarla başlar.",
    ),
    // measure
    ("App.Measure.Scope", "Oscilloscope", "Oszilloskop", "Osiloskop"),
    ("App.Measure.ScopeDemo", "Demo mode", "Demo-Modus", "Demo modu"),
    ("App.Measure.ScopeConnected", "Connected", "Verbunden", "Bağlı"),
    ("App.Measure.ScopeNotReady", "not ready", "nicht bereit", "hazır değil"),
    ("App.Measure.Device", "Device", "Gerät", "Cihaz"),
    ("App.Measure.State", "State", "Zustand", "Durum"),
    ("App.Measure.Running", "running", "läuft", "çalışıyor"),
    ("App.Measure.Stopped", "stopped", "gestoppt", "durduruldu"),
    (
        "App.Measure.StartStop.ToolTip",
        "Start / stop data acquisition of the oscilloscope",
        "Datenerfassung des Oszilloskops starten / stoppen",
        "Osiloskobun veri toplamasını başlat / durdur",
    ),
    (
        "App.Measure.ShowGui.ToolTip",
        "Show the oscilloscope window of the vendor software",
        "Das Oszilloskop-Fenster der Herstellersoftware anzeigen",
        "Üretici yazılımının osiloskop penceresini göster",
    ),
    ("App.Measure.Record", "Record", "Aufnahme", "Kayıt"),
    (
        "App.Measure.ScopeNotRunning",
        "The oscilloscope is not running – press \"start Oszi\" first.",
        "Das Oszilloskop läuft nicht – zuerst „start Oszi“ drücken.",
        "Osiloskop çalışmıyor – önce \"Oszi Çalıştır\" düğmesine basın.",
    ),
    ("App.Measure.DelayHint", "Record delay: {0} s", "Aufnahmeverzögerung: {0} s", "Kayıt gecikmesi: {0} s"),
    ("App.Measure.Countdown", "Recording in {0} s", "Aufnahme in {0} s", "{0} s içinde kayıt"),
    ("App.Measure.Position", "Position", "Position", "Konum"),
    ("App.Measure.LastPoint", "Last point of the row", "Letzter Punkt der Reihe", "Sıranın son noktası"),
    ("App.Measure.NoPoint", "No point yet", "Noch kein Punkt", "Henüz nokta yok"),
    ("App.Measure.NotRecorded", "not recorded yet", "noch nicht aufgenommen", "henüz kaydedilmedi"),
    ("App.Measure.Time", "Recorded", "Aufgenommen", "Kayıt zamanı"),
    ("App.Measure.Source", "Source", "Quelle", "Kaynak"),
    ("App.Measure.SampleRate", "Sample rate", "Abtastrate", "Örnekleme hızı"),
    ("App.Measure.DataValid", "Point data valid", "Punktdaten gültig", "Nokta verileri geçerli"),
    // survey
    ("App.Survey.General", "General", "Allgemein", "Genel"),
    ("App.Survey.Now", "Now", "Jetzt", "Şimdi"),
    (
        "App.Survey.Now.ToolTip",
        "Insert the current date and time",
        "Aktuelles Datum und Uhrzeit einsetzen",
        "Geçerli tarih ve saati gir",
    ),
    ("App.Survey.Geometry", "Grid", "Raster", "Izgara"),
    ("App.Survey.Parameters", "Measurement parameters", "Messparameter", "Ölçüm parametreleri"),
    ("App.Survey.Custom", "Custom", "Benutzerdefiniert", "Özel"),
    (
        "App.Survey.dfh.ToolTip",
        "Δfh – frequency difference constant used in the depth calculation",
        "Δfh – Frequenzdifferenz-Konstante der Tiefenberechnung",
        "Δfh – derinlik hesaplamasında kullanılan frekans farkı sabiti",
    ),
    (
        "App.Survey.PendingEdits",
        "Changes not applied yet – press Apply or Enter.",
        "Änderungen noch nicht übernommen – „Übernehmen“ oder Eingabetaste drücken.",
        "Değişiklikler henüz uygulanmadı – Uygula düğmesine veya Enter tuşuna basın.",
    ),
    ("App.Survey.Derived", "Calculated values", "Berechnete Werte", "Hesaplanan değerler"),
    ("App.Survey.DepthPerSample", "Depth / sample", "Tiefe / Sample", "Derinlik / örnek"),
    ("App.Survey.MaxDepth", "Depth at 2048", "Tiefe bei 2048", "2048'deki derinlik"),
    ("App.Survey.FreqStep", "Frequency step", "Frequenzschritt", "Frekans adımı"),
    (
        "App.Survey.DepthFormula",
        "t(i) = 100·Kb·Tm·(500·f·i/2048) / (2·Δfh)",
        "t(i) = 100·Kb·Tm·(500·f·i/2048) / (2·Δfh)",
        "t(i) = 100·Kb·Tm·(500·f·i/2048) / (2·Δfh)",
    ),
    // display
    ("App.Display.Zoom", "Zoom", "Zoom", "Yakınlaştırma"),
    ("App.Display.ZoomX", "Zoom X (points)", "Zoom X (Punkte)", "Zoom X (noktalar)"),
    ("App.Display.ZoomY", "Zoom Y (samples)", "Zoom Y (Samples)", "Zoom Y (örnekler)"),
    ("App.Display.ZoomZ", "Zoom Z (rows)", "Zoom Z (Reihen)", "Zoom Z (sıralar)"),
    ("App.Display.Processing", "Processing", "Verarbeitung", "İşleme"),
    ("App.Display.BlockSize", "Block size", "Blockgröße", "Blok boyutu"),
    ("App.Display.HorizontalCut", "Horizontal cut", "Horizontalschnitt", "Yatay kesit"),
    ("App.Display.Sample", "Sample", "Sample", "Örnek"),
    ("App.Display.ValidPoints", "Valid points", "Gültige Punkte", "Geçerli noktalar"),
    ("App.Display.ValidBase", "Valid base", "Gültige Basis", "Geçerli taban"),
    ("App.Display.ValidStep", "Valid step", "Gültiger Schritt", "Geçerli adım"),
    ("App.Display.Colors", "Colours", "Farben", "Renkler"),
    ("App.Display.ReverseColors", "Reverse colours", "Farben umkehren", "Renkleri ters çevir"),
    ("App.Display.ColorBase", "Colour base = {0}", "Farbbasis = {0}", "Renk tabanı = {0}"),
    ("App.Display.ColorBaseReset", "Reset", "Zurücksetzen", "Sıfırla"),
    // data
    ("App.Data.Rows", "Rows", "Reihen", "Sıralar"),
    ("App.Data.Row", "Row", "Reihe", "Sıra"),
    ("App.Data.Points", "Points", "Punkte", "Noktalar"),
    ("App.Data.Point", "Point", "Punkt", "Nokta"),
    ("App.Data.PointsShort", "pts", "Pkt.", "nkt."),
    ("App.Data.NoRows", "No rows", "Keine Reihen", "Sıra yok"),
    ("App.Data.NoPoints", "No points in this row", "Keine Punkte in dieser Reihe", "Bu sırada nokta yok"),
    ("App.Data.PrevPoint", "Previous point", "Vorheriger Punkt", "Önceki nokta"),
    ("App.Data.NextPoint", "Next point", "Nächster Punkt", "Sonraki nokta"),
    (
        "App.Data.ConfirmDeleteAll",
        "Delete all {0} rows with {1} points? This cannot be undone.",
        "Alle {0} Reihen mit {1} Punkten löschen? Dies kann nicht rückgängig gemacht werden.",
        "{1} noktalı {0} sıranın tümü silinsin mi? Bu işlem geri alınamaz.",
    ),
    ("App.Data.MinMax", "Min / Max (current row)", "Min / Max (aktuelle Reihe)", "Min / Max (geçerli sıra)"),
    ("App.Data.Minimum", "Minimum", "Minimum", "Minimum"),
    ("App.Data.Maximum", "Maximum", "Maximum", "Maksimum"),
    (
        "App.Data.MinMax.Hint",
        "Fits the colour scale to the values of the current row.",
        "Passt die Farbskala an die Werte der aktuellen Reihe an.",
        "Renk skalasını geçerli sıranın değerlerine uyarlar.",
    ),
    // messages
    ("App.Messages.Level", "Level", "Stufe", "Seviye"),
    ("App.Messages.Level.Error", "Errors", "Fehler", "Hatalar"),
    ("App.Messages.Level.Warn", "Warnings", "Warnungen", "Uyarılar"),
    ("App.Messages.Level.Info", "Info", "Info", "Bilgi"),
    ("App.Messages.Level.Debug", "Debug", "Debug", "Hata ayıklama"),
    ("App.Messages.Level.Trace", "All", "Alle", "Tümü"),
    ("App.Messages.Debug", "Debug", "Debug", "Debug"),
    (
        "App.Messages.Debug.ToolTip",
        "Write detailed debug messages (Tracer debug)",
        "Ausführliche Debug-Meldungen schreiben (Tracer debug)",
        "Ayrıntılı hata ayıklama iletileri yaz (Tracer debug)",
    ),
    ("App.Messages.Count", "{0} of {1} messages", "{0} von {1} Meldungen", "{1} iletiden {0}"),
    ("App.Messages.CopyAll", "Copy all", "Alles kopieren", "Tümünü kopyala"),
    ("App.Messages.Copied", "Messages copied to the clipboard", "Meldungen in die Zwischenablage kopiert", "İletiler panoya kopyalandı"),
    ("App.Messages.Clear", "Clear", "Leeren", "Temizle"),
    ("App.Messages.Empty", "No messages", "Keine Meldungen", "İleti yok"),
    // help
    ("App.Help.Search", "Search help", "Hilfe durchsuchen", "Yardımda ara"),
    ("App.Help.NoResults", "No matching topics", "Keine passenden Themen", "Eşleşen konu yok"),
];

pub(crate) fn fallback(lang: Language, key: &str) -> Option<&'static str> {
    TABLE.iter().find(|e| e.0 == key).map(|e| match lang {
        Language::English => e.1,
        Language::German => e.2,
        Language::Turkish => e.3,
    })
}

/// Iterator over all entries (for the NOTES file / tests).
#[cfg(test)]
pub(crate) fn all() -> impl Iterator<Item = &'static (&'static str, &'static str, &'static str, &'static str)> {
    TABLE.iter()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_unique_and_prefixed() {
        let mut seen = std::collections::HashSet::new();
        for e in all() {
            assert!(e.0.starts_with("App."), "{}", e.0);
            assert!(seen.insert(e.0), "duplicate {}", e.0);
            assert!(!e.1.is_empty() && !e.2.is_empty() && !e.3.is_empty());
        }
    }

    #[test]
    fn every_used_key_has_a_fallback() {
        // all App.* keys referenced by panels/ and help/
        let sources = [
            include_str!("mod.rs"),
            include_str!("files.rs"),
            include_str!("measure.rs"),
            include_str!("survey.rs"),
            include_str!("display.rs"),
            include_str!("data.rs"),
            include_str!("messages.rs"),
            include_str!("../help/mod.rs"),
        ];
        for src in sources {
            for (i, _) in src.match_indices("\"App.") {
                let rest = &src[i + 1..];
                let end = rest.find('"').unwrap();
                let key = &rest[..end];
                assert!(fallback(Language::English, key).is_some(), "missing fallback for {key}");
            }
        }
    }
}
