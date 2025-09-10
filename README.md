# Kleinanzeigen Telegram Bot (Rust-Version)

Dies ist ein kleiner, effizienter Bot, der Kleinanzeigen nach neuen "Zu verschenken"-Angeboten in einem bestimmten Bereich durchsucht und Benachrichtigungen an eine Telegram-Gruppe sendet. Er ist dafür konzipiert, als `systemd`-Dienst unter Linux zu laufen.

-----

## Voraussetzungen

Bevor Sie beginnen, stellen Sie sicher, dass die folgende Software installiert ist und Sie die notwendigen Telegram-Zugangsdaten haben.

### Software-Installation

  * **Build-Abhängigkeiten**
    Erforderlich für das Kompilieren von Rust-Projekten, die C-Bibliotheken wie OpenSSL verwenden.

      * **Für Debian/Ubuntu-basierte Systeme:**
        ```bash
        sudo apt install build-essential pkg-config libssl-dev
        ```
      * **Für Arch Linux-basierte Systeme:**
        ```bash
        sudo pacman -Syu base-devel pkg-config openssl
        ```

  * **Rust Toolchain (inkl. Cargo)**
    Der empfohlene Weg, um die aktuellste Version zu erhalten, ist die Installation über die Paketquellen Ihrer Distribution:

      * **Für Debian/Ubuntu-basierte Systeme:**
        ```bash
        sudo apt update && sudo apt install cargo
        ```
      * **Für Arch Linux-basierte Systeme:**
        ```bash
        sudo pacman -Syu rust
        ```

  * **Git**
    Wird benötigt, um das Projekt-Repository zu klonen.

      * **Für Debian/Ubuntu-basierte Systeme:**
        ```bash
        sudo apt update && sudo apt install git
        ```
      * **Für Arch Linux-basierte Systeme:**
        ```bash
        sudo pacman -Syu git
        ```

### Telegram-Zugangsdaten

  * **Ein Telegram-Konto:** Um einen Bot und eine Gruppe zu erstellen.
  * **Einen Telegram Bot-Token & Gruppen-Chat-ID:** Befolgen Sie die nachstehenden Schritte, um diese zu erhalten.

-----

### So erhalten Sie Ihre Telegram-Zugangsdaten

**1. Telegram Bot-Token erhalten:**

  * Öffnen Sie Telegram und suchen Sie nach dem Benutzer **`@BotFather`**.
  * Starten Sie einen Chat mit `@BotFather` und senden Sie den Befehl `/newbot`.
  * Folgen Sie den Anweisungen auf dem Bildschirm, um einen Namen und einen Benutzernamen für Ihren Bot zu wählen.
  * `@BotFather` wird Ihnen einen einzigartigen **API-Token** zur Verfügung stellen, der etwa so aussieht: `1234567890:ABC-DEF1234ghIkl-799jL_L2345`. **Speichern Sie diesen Token sicher.**

**2. Gruppen-Chat-ID erhalten:**

  * Erstellen Sie eine neue Telegram-Gruppe.
  * Fügen Sie Ihren neu erstellten Bot zu dieser Gruppe hinzu.
  * **Wichtig:** Befördern Sie den Bot zum **Administrator** der Gruppe.
  * Senden Sie eine beliebige Nachricht in der Gruppe (z. B. "Hallo").
  * Öffnen Sie Ihren Webbrowser und gehen Sie zu dieser URL, wobei Sie `<YOUR_BOT_TOKEN>` durch Ihren gespeicherten Token ersetzen:
    `https://api.telegram.org/bot<YOUR_BOT_TOKEN>/getUpdates`
  * Sie werden eine JSON-Antwort sehen. Suchen Sie nach einem Abschnitt, der wie folgt aussieht: `{"update_id":...,"message":{..."chat":{"id":-1234567890,"title":"..."}}}`.
  * Der `id`-Wert (z. B. `-1234567890`) ist Ihre **Gruppen-Chat-ID**. Es muss eine negative Zahl sein. **Speichern Sie diese ID.**

-----

## Installation und Einrichtung

### Schritt 1: Repository klonen

Laden Sie zunächst die Projektdateien mit `git` von GitHub herunter.

```bash
git clone https://github.com/loxoron218/kleinanzeigen-telegram-rust-bot.git
```

### Schritt 2: Projektdateien platzieren

Verschieben Sie den heruntergeladenen Projektordner (`kleinanzeigen-telegram-rust-bot`) an seinen endgültigen Bestimmungsort. Diese Anleitung geht davon aus, dass Sie den folgenden Pfad verwenden:

```bash
# Verzeichnis erstellen, falls es nicht existiert
mkdir -p ~/.local/share/

# Ihr Projekt dorthin verschieben
mv kleinanzeigen-telegram-rust-bot ~/.local/share/
```

### Schritt 3: Bot konfigurieren

Navigieren Sie zum Projektverzeichnis und öffnen Sie die Hauptquelldatei, um Ihre Zugangsdaten hinzuzufügen.

```bash
cd ~/.local/share/kleinanzeigen-telegram-rust-bot/
nano src/main.rs
```

Ersetzen Sie in der Datei die Platzhalterwerte für `TELEGRAM_BOT_TOKEN` und `TELEGRAM_CHAT_ID` durch Ihre tatsächlichen Zugangsdaten.

### Schritt 4: Release-Binary kompilieren

Kompilieren Sie nun die endgültige, optimierte Version des Bots. Dieser Befehl muss aus dem Projektverzeichnis heraus ausgeführt werden.

```bash
cargo build --release
```

Dadurch wird die ausführbare Datei unter `~/.local/share/kleinanzeigen-telegram-rust-bot/target/release/kleinanzeigen-telegram-rust-bot` erstellt.

-----

## Einrichtung als Systemd-Dienst

Dadurch wird der Bot automatisch im Hintergrund ausgeführt und beim Systemstart gestartet.

### Schritt 1: Service-Datei erstellen

Erstellen Sie eine neue systemd-Service-Datei mit einem Texteditor mit `sudo`-Berechtigungen:

```bash
sudo nano /etc/systemd/system/kleinanzeigen.service
```

### Schritt 2: Service-Konfiguration hinzufügen

Kopieren Sie die folgende Konfiguration und fügen Sie sie in die Datei ein. **Denken Sie daran, `user` durch Ihren tatsächlichen Benutzernamen zu ersetzen.**

```ini
[Unit]
Description=Kleinanzeigen Telegram Bot

[Service]
# 'user' durch Ihren tatsächlichen Benutzernamen ersetzen
User=user

# Das Arbeitsverzeichnis für die Anwendung festlegen
WorkingDirectory=/home/user/.local/share/kleinanzeigen-telegram-rust-bot

# Pfad zum kompilierten Binary
ExecStart=/home/user/.local/share/kleinanzeigen-telegram-rust-bot/target/release/kleinanzeigen-telegram-rust-bot
```

**Hinweis:** `~` funktioniert in systemd-Service-Dateien nicht, daher müssen Sie den vollständigen Pfad wie `/home/user/` verwenden.

### Schritt 3: Timer-Konfiguration hinzufügen

Erstellen Sie als Nächstes die entsprechende Timer-Unit-Datei mit Root-Rechten:

```bash
sudo nano /etc/systemd/system/kleinanzeigen.timer
```

Fügen Sie den folgenden Inhalt in die Datei ein:

```ini
[Unit]
Description=Run Kleinanzeigen Telegram Bot every 5 minutes

[Timer]
# Definiert die Zeit für den Start des Timers
OnBootSec=1min
# Definiert das Intervall, in dem der Timer neu gestartet wird
OnUnitActiveSec=5min

[Install]
WantedBy=timers.target
```

### Schritt 4: Dienst und Timer aktivieren und starten

Führen Sie die folgenden Befehle aus, um Ihren neuen Dienst und Timer zu installieren und zu starten:

```bash
# systemd neu laden, um die neue Datei zu erkennen
sudo systemctl daemon-reload

# Dienst und Timer für den Start beim Booten aktivieren
sudo systemctl enable --now kleinanzeigen.service kleinanzeigen.timer
```

-----

## Überprüfung des Dienstes

Sie können Ihren Bot jederzeit mit diesen Befehlen überprüfen:

  * **Status prüfen:**
    ```bash
    sudo systemctl status kleinanzeigen.service
    ```
  * **Live-Logs zur Fehlersuche anzeigen:**
    ```bash
    journalctl -u kleinanzeigen.service -f
    ```
