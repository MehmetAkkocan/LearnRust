# Rust Öğrenme Uygulaması

Rust'ı adım adım, tarayıcıda yazdığın kodla öğrenmeni sağlayan yerel bir web uygulaması (Axum). Öğrenci kodu konteyner içinde `rustc` ile derlenir ve çalıştırılır; ana makinede Rust kurulumu gerekmez.

## Gereksinimler

- Windows 11 + [Podman Desktop](https://podman-desktop.io/) (WSL makinesi çalışır durumda olmalı)
- `podman compose` (Podman Desktop ile gelir) veya `podman-compose`

## Hızlı başlangıç

```powershell
.\run.ps1
```

İmaj derlenir, konteyner başlatılır ve `http://localhost:8080` adresi yazdırılır. Diğer seçenekler: `-Build`, `-Up`, `-Down`, `-Logs`, `-Check`, `-Shell`.

## İçerik yapısı

Dersler `content/` altındadır. Biçim ve alanlar için bkz. [content/SCHEMA.md](content/SCHEMA.md).

## Dil (Türkçe / English)

Sağ üstteki bayraklı menüden dil seçilir; seçim `lang` çerezinde saklanır. Adresler ve ilerleme iki dilde ortaktır. Türkçe dosyalar asıl kaynaktır; İngilizce metinler `content/en/levelN/` altında aynı adlı dosyalarda durur (bkz. SCHEMA.md, "İngilizce içerik"). Arayüz metinleri `src/i18n.rs` içindedir.

## Puan ve kilit sistemi (kısa)

Her görev doğru çözüldükçe puan kazandırır. Sonraki ders/seviye, gereken puana ulaşılana kadar kilitli kalır.

## İlerleme dosyası ve yedekleme

İlerleme `data/progress.json` dosyasında tutulur (`./data` konteynere bind mount edilir). Yedeklemek için dosyayı kopyalaman yeterli; geri yüklemek için aynı yere koy. Sıfırlamak için dosyayı sil.

## İçerik kontrolü

```powershell
.\run.ps1 -Check
```

`/app/rust_learning_app --check-content` çalıştırır ve içerik dosyalarını doğrular.

## Güvenlik

Port yalnızca yerel erişime açıktır (`127.0.0.1:8080`); ağdaki diğer cihazlar erişemez. Konteyner sınırlıdır: salt okunur kök dosya sistemi, `/tmp` için tmpfs, tüm capability'ler kapalı, bellek/CPU/süreç limitleri. **Ancak öğrenci kodu dış ağa erişebilir.** Ağı tamamen kapatmak port yayınını da kapatacağından kapatılmamıştır. Uygulamayı yalnızca kendi makinende kullan, internete açma.

## WSL bind mount notu

`./data` klasörü WSL üzerinden bağlanır. Yazma izni sorunu olursa:

- `compose.yaml` içindeki `:Z` etiketini koru;
- `compose.yaml` içinde `# userns_mode: keep-id` satırının başındaki `#` işaretini kaldır (CLI ile: `podman run --userns=keep-id ...`).

## Sorun giderme

- İlk build uzun sürer (bağımlılıklar indirilip derlenir); sonrakiler önbellekten hızlıdır.
- Git Bash kullanıyorsan Podman komutlarından önce `export MSYS_NO_PATHCONV=1` yaz.
- `podman compose` bulunamazsa Podman Desktop'u güncelle veya `podman-compose` kur.
- 8080 doluysa `compose.yaml` içindeki port eşlemesini değiştir (ör. `127.0.0.1:8081:8080`).
- Rust imajı `1.99-slim` etiketine sabitlidir; güncellemek için `Containerfile` içindeki iki `FROM` satırını değiştir.
