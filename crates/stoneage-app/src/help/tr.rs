//! Türkçe kullanım kılavuzu.

use super::Topic;

pub(super) const TOPICS: &[Topic] = &[
    Topic {
        id: "start",
        title: "Başlarken",
        body: r#"
StoneAge, Velleman PCSGU250 USB osiloskobu ile frekans sondajları kaydeder ve bunlardan derinlik kesitleri, yatay kesitler, haritalar ve yer altının 3B bloğunu oluşturur.

# Pencere
- Solda: Dosyalar, Kayıt, Ölçüm, Görünüm, Veri, Yardım ve Mesajlar bölümlerini içeren kenar çubuğu. Bir simgeye tıklamak bölümü değiştirir, etkin simgeye tekrar tıklamak paneli kapatır.
- Üstte: menü çubuğu (orijinal programın tüm menü öğeleri) ve Harita, 3D, Arena, Sıra, Nokta, SesTesti ve SesTesti2 sekmeleri.
- Altta: << >> düğmeleriyle "Sıra x / y" ve "Nokta x / y", osiloskop durumu, fare altındaki değerler ve "değiştirildi" göstergesi bulunan durum çubuğu.

# Hızlı başlangıç
- Osiloskobu ve sondayı bağlayın, Kayıt bölümünde "Oszi Çalıştır" düğmesine basın.
- Ölçüm bölümünde başlığı, nokta ve sıra mesafesini girin, Kb ve f frekans alanını seçin ve "Ayarları Kaydet" (Uygula) düğmesine basın.
- İlk noktaya gidin ve Kayıt (veya Boşluk tuşu) düğmesine basın. Her kayıt geçerli noktayı saklar ve yeni, boş bir nokta ekler.
- Sıranın sonunda "Kayıt + Yeni Sıra" kullanın: nokta kaydedilir ve yeni bir noktayla yeni bir sıra başlatılır.
- Ölçümü Ctrl+S ile kaydedin (*.saf).

> Osiloskop olmadan StoneAge demo modunda çalışır ve sentetik veri üretir; böylece tüm işlevler denenebilir.
"#,
    },
    Topic {
        id: "workflow",
        title: "Ölçüm akışı",
        body: r#"
Bir ölçüm (*.saf dosyası) sıralardan, her sıra da noktalardan oluşur. Noktalar sabit nokta mesafesiyle bir hat boyunca, sıralar ise sabit sıra mesafesiyle paralel olarak kaydedilir. Böylece Harita, Arena ve 3D görünümlerinin gösterdiği düzenli bir ızgara oluşur.

# 1. Hazırlık
- Yeni bir ölçüm oluşturun (Dosyalar → Yeni, Ctrl+N). Parametreler StoneAge.ini dosyasındaki varsayılanlardan alınır.
- Ölçüm bölümü: Başlık, Tarih ("Şimdi" geçerli tarih ve saati girer) ve Not girin.
- Nokta mesafesini ve sıra mesafesini metre olarak girin.
- Kb (zemin cinsi) ve f frekans alanını seçin. f, osiloskopta ayarlanan frekans alanıyla aynı olmalıdır.
- delta (Δfh) ve Tm değerlerini kontrol edin. Uygula düğmesine basın.

# 2. Osiloskobu başlatma
- Kayıt bölümü → "Oszi Çalıştır". Durum kartı yeşile döner ("Bağlı").
- "Oszi Göster", sinyali, volt/bölme ve zaman tabanını kontrol etmek için üreticinin osiloskop penceresini açar.

# 3. Kayıt
- Sondayı ilk noktaya yerleştirin ve Kayıt (Boşluk) düğmesine basın.
- Bir nokta mesafesi ilerleyin ve tekrar Kayıt düğmesine basın – ve bu şekilde devam edin.
- "Gecikmeli Kayıt" (Ctrl+Boşluk), StoneAge.ini dosyasında ayarlanan gecikmeyi (RecordDelay, varsayılan 5 s) bekler – önce sondaya yürümeniz gerektiğinde kullanışlıdır. Geri sayım gösterilir; İptal ile durdurulur.
- Sıranın sonunda: "Kayıt + Yeni Sıra" noktayı kaydeder ve sonraki sırayı başlatır.

# 4. Kontrol
- Nokta görünümü: yeni kaydedilen noktanın spektrumu. "Nokta verileri geçersiz …" uyarısı verinin kullanılamaz olduğunu gösterir; noktayı yeniden kaydedin.
- Sıra görünümü: geçerli sıranın kesiti her noktayla büyür.

# 5. Değerlendirme
- Arena ve Harita yatay kesitleri gösterir; Görünüm bölümündeki yatay kesit kaydırıcısını hareket ettirin.
- Yakınlaştırma, blok boyutu, interpolasyon ve renkleri Görünüm bölümünde ayarlayın.
- "Bul Min/Max" renk skalasını sıranın verilerine uyarlar.

# 6. Kaydetme ve dışa aktarma
- Kaydet (Ctrl+S) veya Farklı Kaydet (Ctrl+Shift+S).
- Tablolar için CSV dışa aktarma, raporlar için görüntü dışa aktarma, yazdırma.
"#,
    },
    Topic {
        id: "parameters",
        title: "Parametreler ve formüller",
        body: r#"
Her nokta 2048 örnek içerir (indeks i = 0 … 2047) – osiloskoptan okunan spektrum. Sıra, Arena ve 3D görünümlerinin dikey ekseni bu indekstir; frekans veya hesaplanan derinlik olarak gösterilir.

# Kb – zemin sabiti
Ölçüm alanındaki zemine bağlıdır. Liste -1 … 13 zemin cinslerini sunar; parantez içindeki sayı Kb × 1000'dir (örn. "Zemin Cinsi 6 (1100)" = Kb 1,1). "Özel" seçeneğiyle herhangi bir değer (× 1000) girilebilir. Hesaplanan derinlik Kb ile orantılıdır.

# f – frekans alanı [kHz]
Osiloskobun frekans alanı: 30, 60, 120, 300 kHz veya 0,6, 1,2, 3, 6 MHz. Osiloskoptaki ayarla aynı olmalıdır, aksi halde tüm derinlikler yanlış olur. Her nokta ayrıca kaydedildiği andaki alanı saklar.

# delta – Δfh
Yöntemin frekans farkı sabiti (varsayılan 400). Derinlik Δfh ile ters orantılıdır.

# Tm [s]
Ölçümün zaman sabiti (varsayılan 0,02 s). Ölçüm dosyasında saklanır; derinlik Tm ile orantılıdır.

# i örneğinin derinliği
Orijinal programdaki gibi hesaplanır (MessReihe.tiefe):
$ f(i) = 500 · fRange / 2048 · i
$ t(i) = 100 · Kb · Tm · f(i) / (2 · Δfh)
Sonuç santimetreye yuvarlanır. Kısaca:
$ t(i) = 25000 · Kb · Tm · fRange · i / (2048 · Δfh)   [m]
Varsayılan değerlerle örnek (Kb = 1,1, Tm = 0,02, fRange = 300, Δfh = 400): örnek başına 0,20 m, 512. örnekte 103,13 m ve 2048. örnekte 412,5 m.

# Frekans etiketleri
Frekans ekseni (MessReihe.freq) noktada saklanan alanı ve görünüm ayarlarını kullanır:
$ f = round(0,1 · BlockSize · (y / ZoomY) · fRange / 2048) / 100   [MHz]
y piksel konumudur. Bir örneğin frekans adımı fRange / 2048'dir.

# Nokta ve sıra mesafesi [m]
Bir sıradaki komşu noktalar ve komşu sıralar arasındaki mesafe. Harita, Arena ve 3D görünümlerindeki x ve z konumlarını belirler.

# Uygula
Metin alanları, alanda Enter tuşuna veya Uygula düğmesine basıldığında geçerli olur. Açılır listeler, onay kutuları ve kaydırıcılar hemen etki eder – ve orijinaldeki gibi her değişiklik tüm alanları birlikte uygular. Geçersiz sayılar reddedilir, eski değer geri yüklenir ve Mesajlar bölümüne bir ileti yazılır.
"#,
    },
    Topic {
        id: "recording",
        title: "Kayıt",
        body: r#"
# Düğmeler
| Kayıt | geçerli noktayı kaydeder ve yeni bir nokta ekler (Boşluk) |
| Kayıt + Yeni Sıra | geçerli noktayı kaydeder ve yeni bir noktayla yeni bir sıra oluşturur (Shift+Boşluk) |
| Gecikmeli Kayıt | Kayıt gibi, kayıt gecikmesinden sonra (Ctrl+Boşluk) |
| Gecikmeli Kayıt + Yeni Sıra | Kayıt + Yeni Sıra gibi, kayıt gecikmesinden sonra |

Geçerli sıranın son noktası her zaman bir sonraki kaydın dolduracağı "açık" noktadır. Bu nokta zaten kaydedilmişse yeni bir nokta oluşturulur ve kaydedilir.

# Gecikme
Gecikme, StoneAge.ini dosyasındaki RecordDelay değeridir (milisaniye, varsayılan 5000). Geri sayım sırasında bir ilerleme çubuğu gösterilir; İptal bekleyen kaydı iptal eder.

# Veri kontrolü
Kayıttan sonra veriler kontrol edilir (MessPunkt.isDataValid). Geçersizse Kayıt bölümünde ve Mesajlar'da "Nokta verileri geçersiz – Kablo ve cihazı inceleyin…" görünür ve nokta Veri bölümünde ⚠ ile işaretlenir.

# Osiloskop
- Oszi Çalıştır / Oszi Durdur, PCSGU250'nin veri toplamasını başlatır ve durdurur.
- Oszi Göster, üreticinin osiloskop penceresini gösterir veya gizler (volt/bölme, zaman tabanı, tetikleme).
- Demo modunda sentetik ama gerçekçi veriler üretilir.
"#,
    },
    Topic {
        id: "views",
        title: "Görünümler",
        body: r#"
Görünümler üstteki sekmelerle veya Ctrl+1 … Ctrl+7 ile değiştirilir.

# Harita (Ctrl+1)
Ölçüm alanının üstten görünümü. Her nokta kendi konumunda (nokta mesafesi × sıra mesafesi) çizilir ve yatay kesit derinliğindeki değere göre renklendirilir. Izgaranın altına bir harita yerleştirilebilir (Dosyalar → Haritayı Yükle, JPG/GIF/PNG); konum, ölçek ve dönüş ölçümde saklanır.

# 3D (Ctrl+2)
Tüm ölçüm üç boyutlu bir blok olarak: x = noktalar, y = derinlik/frekans, z = sıralar. Döndürmek için fareyle sürükleyin, yakınlaştırmak için tekerleği kullanın. Görünüm bölümündeki Zoom X/Y/Z eksenleri ölçekler.

# Arena (Ctrl+3)
Tüm sıralar yan yana, yatay kesit olarak ("Kesilen derinlik … m"). Birkaç sıra boyunca uzanan yapıları tanımak için uygundur.

# Sıra (Ctrl+4)
Geçerli sıranın dikey kesiti: x = noktanın metre cinsinden konumu, y = frekans veya ("Derinliği hesapla" ile) derinlik, renk = genlik. Yatay çizgi yatay kesiti gösterir. Fare imleci değeri, frekansı/derinliği ve konumu gösterir.

# Nokta (Ctrl+5)
Geçerli noktanın spektrumu (veya sinyali); kaynak (spektrum/sinyal), frekans alanı ve sinyal uzunluğu ile. Her kaydı kontrol etmek için kullanılır.

# SesTesti / SesTesti2 (Ctrl+6, Ctrl+7)
Sinyali bilgisayarın ses girişinden alır – sonda ve kabloyu osiloskop olmadan kontrol etmek için.

# Renk açıklaması
Ölçek çubuğu renk modelini gösterir; sürüklemek renk tabanını kaydırır. Görünüm → Renkler modeli seçer ve tersine çevirir.
"#,
    },
    Topic {
        id: "display",
        title: "Görünüm ayarları",
        body: r#"
- Zoom X – bir noktanın ekrandaki genişliği.
- Zoom Y – bir ölçüm örneğinin yüksekliği.
- Zoom Z – bir sıranın genişliği (3D/Arena).
- Blok boyutu – bir blokta birleştirilen örnek sayısı (1 … 32).
- Blok-ortalama değeri – tek örnekler yerine bloğun örneklerinin ortalamasını kullanır.
- İnterpolasyon – noktalar arasında yumuşak renk geçişleri.
- Kareleri göster – ölçüm ızgarasını çizer.
- Derinliği hesapla – dikey ekseni frekans yerine hesaplanan derinlikle etiketler.
- Yatay kesit – Harita/Arena'da gösterilen derinlik kesitinin örnek indeksi (0 … 2048); kaydırıcının altındaki etiketler derinliği metre olarak gösterir.
- Geçerli taban / adım – yalnızca StoneAge.ini'de showValidBase = true ise görünür: sıranın hangi noktalarının geçerli sayılacağı (ilk geçerli nokta ve geçerli noktalar arasındaki mesafe).
- Renk modeli – palet seçimi; "Renkleri ters çevir" paleti tersine çevirir.

Tüm görünüm ayarları ölçüm dosyasında saklanır.
"#,
    },
    Topic {
        id: "data",
        title: "Verileri düzenleme",
        body: r#"
Veri bölümü tüm sıraları nokta sayılarıyla, geçerli sıranın noktalarını da kayıt zamanı ve durumuyla (✔ geçerli, ⚠ geçersiz, ○ henüz kaydedilmedi) listeler. Bir girişe tıklamak onu geçerli yapar.

- Yeni Sıra Ekle – geçerli sıradan sonra boş bir sıra ekler.
- Geçerli Sırayı Sil / Geçerli Noktayı Sil.
- Tüm Sıraları Sil – onay ister; geri alınamaz (kaydetmemek dışında).
- Bul Min/Max – geçerli sıranın minimum ve maksimumunu belirler ve renk skalasını uyarlar.
- Yenile Min/Max – tam aralığa geri döner.

Gezinme: durum çubuğundaki << >>, ok tuşları (← → nokta, ↑ ↓ sıra) veya Düzenle → Önceki/Sonraki Sıra.
"#,
    },
    Topic {
        id: "files",
        title: "Dosyalar ve biçimler",
        body: r#"
# StoneAge dosyaları (*.saf)
Tüm parametreler, görünüm ayarları, harita yerleşimi ve tüm sıra ve noktalarıyla ölçüm. Biçim tanımlayıcısı: {format}. Orijinal StoneAge dosyaları açılabilir ve aynı biçimde yazılır.

# CSV
CSV dışa aktarma ölçüm değerlerini tablo olarak yazar; CSV alma böyle bir tabloyu geri okur.

# Harita görüntüleri
Haritayı Yükle JPG, GIF ve PNG kabul eder. Görüntünün kendisi .saf dosyasında saklanmaz – yalnızca yerleşimi.

# Görüntü dışa aktarma / Yazdırma
Geçerli görünümü görüntü dosyası olarak dışa aktarın veya yazdırın.

# StoneAge.ini – varsayılanlar
Language, Country, RecordDelay, PointDistance, RowDistance, dfh, Kb, fRange, disp_stepx/y/z, colorModel, colorModelBase ve showValidBase. "Ana Ayarları Kaydet" geçerli ölçümün parametrelerini bu dosyaya yazar; yeni ölçümler bunlarla başlar.

# Son kullanılanlar
Dosyalar bölümü son kullanılan dosyaları listeler; tıklamak dosyayı açar. Artık var olmayan dosyaların üzeri çizilidir.
"#,
    },
    Topic {
        id: "shortcuts",
        title: "Klavye kısayolları",
        body: r#"
| Ctrl+N | Yeni ölçüm |
| Ctrl+O | Aç |
| Ctrl+S | Kaydet |
| Ctrl+Shift+S | Farklı kaydet |
| Ctrl+P | Yazdır |
| Ctrl+Q | Çıkış |
| Boşluk | Kayıt |
| Ctrl+Boşluk | Gecikmeli kayıt |
| Shift+Boşluk | Kayıt + Yeni Sıra |
| Esc | Gecikmeli kaydı iptal et |
| ← / → | Önceki / sonraki nokta |
| ↑ / ↓ | Önceki / sonraki sıra |
| T | Protokol aç/kapa (mesajlar) |
| F1 | Yardım |
| Ctrl+1 … Ctrl+7 | Harita, 3D, Arena, Sıra, Nokta, SesTesti, SesTesti2 |
| Enter | Metin alanını uygula (Ölçüm) |

> Bir metin alanı klavye odağına sahipken Boşluk ve ok tuşları dikkate alınmaz.
"#,
    },
    Topic {
        id: "troubleshooting",
        title: "Sorun giderme",
        body: r#"
# Osiloskop bağlı olduğu halde durum "Demo"
- PCSGU250.dll yüklenemedi. Nedeni Mesajlar bölümüne yazılır.
- Velleman DLL'i 32 bittir: gerçek osiloskop yalnızca StoneAge'in 32 bit (x86) sürümüyle çalışır.
- PCSGU250.dll program klasöründe (veya bin alt klasöründe) ya da PATH üzerinde olmalıdır.

# Durum "hazır değil"
- USB kablosunu kontrol edin; hub yerine doğrudan bilgisayara bağlayın; başka bir bağlantı noktası deneyin.
- Velleman PCSGU250 USB sürücüsünü (yeniden) yükleyin – Aygıt Yöneticisi cihazı uyarı işareti olmadan göstermelidir.
- Osiloskobu kullanan diğer programları kapatın (örn. PcLab2000) – yalnızca bir program erişebilir.
- "Oszi Durdur" ve tekrar "Oszi Çalıştır" düğmesine basın veya cihazı çıkarıp yeniden takın.

# "Nokta verileri geçersiz – Kablo ve cihazı inceleyin…"
- Sondayı, kabloyu ve toprak bağlantısını kontrol edin.
- Osiloskobun çalıştığından (Oszi Çalıştır) ve sinyalin göründüğünden (Oszi Göster) emin olun.
- f frekans alanı osiloskop ayarıyla eşleşmelidir.
- Noktayı yeniden kaydedin.

# Derinlikler yanlış görünüyor
- Kb, Δfh, Tm ve f frekans alanını kontrol edin; bkz. "Parametreler ve formüller".

# Mesajlar nerede?
Mesajlar bölümü program günlüğünü gösterir. Ayrıntılı çıktı için "Debug" seçeneğini açın; bir sorunu bildirirken "Tümünü kopyala" kullanın.
"#,
    },
    Topic {
        id: "about",
        title: "StoneAge hakkında",
        body: r#"
StoneAge 2 – sürüm {app_version}.
StoneAge {version} (K-S Analysis) programının modern arayüzlü, aslına sadık yeniden uygulaması.

- Orijinal sürüm: {version}
- Dosya biçimi (.saf): {format}
- Osiloskop: Velleman PCSGU250 (USB), PCSGU250.dll üzerinden
- Diller: Almanca, İngilizce, Türkçe

(c) 2001-2009 K-S Analysis, netz-meister.
"#,
    },
    Topic {
        id: "license",
        title: "Lisans",
        body: "Orijinal programın lisans sözleşmesi (Almanca):",
    },
];
