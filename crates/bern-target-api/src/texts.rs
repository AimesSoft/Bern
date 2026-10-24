//! Texts every native runtime shows itself, the per-language tables of
//! runtimes that fill Bern's named placeholders, and the conversion of
//! those placeholders to a platform's positional format specifiers.

use bern_ir::Application;
use bern_protocol::LocalizedText;

/// Strings the native runtimes show themselves, with built-in translations.
/// Every runtime receives all of them. An argument is `%@` in the texts the
/// SwiftUI and Compose runtimes format, which convert it to their own
/// specifier, and a `{name}` placeholder in the texts the GTK, WinUI 3 and
/// Web runtimes fill by name.
pub const FRAMEWORK_TEXTS: &[(&str, &[(&str, &str)])] = &[
    // iOS shows windows other than the primary as sheets, closed with this
    // button, and offers to open the primary window again once it closed.
    (
        "bern.window.done",
        &[
            ("zh-Hans", "完成"),
            ("zh-Hant", "完成"),
            ("en", "Done"),
            ("ja", "完了"),
            ("ko", "완료"),
            ("ru", "Готово"),
            ("de", "Fertig"),
            ("fr", "Terminé"),
            ("es", "Listo"),
            ("pt-BR", "Concluído"),
            ("it", "Fine"),
            ("tr", "Bitti"),
            ("vi", "Xong"),
            ("pl", "Gotowe"),
            ("uk", "Готово"),
            ("id", "Selesai"),
        ],
    ),
    (
        "bern.window.open",
        &[
            ("zh-Hans", "打开“%@”"),
            ("zh-Hant", "打開「%@」"),
            ("en", "Open “%@”"),
            ("ja", "“%@”を開く"),
            ("ko", "‘%@’ 열기"),
            ("ru", "Открыть «%@»"),
            ("de", "„%@“ öffnen"),
            ("fr", "Ouvrir « %@ »"),
            ("es", "Abrir «%@»"),
            ("pt-BR", "Abrir “%@”"),
            ("it", "Apri “%@”"),
            ("tr", "“%@” aç"),
            ("vi", "Mở “%@”"),
            ("pl", "Otwórz „%@”"),
            ("uk", "Відкрити «%@»"),
            ("id", "Buka “%@”"),
        ],
    ),
    (
        "bern.window.close",
        &[
            ("zh-Hans", "关闭"),
            ("zh-Hant", "關閉"),
            ("en", "Close"),
            ("ja", "閉じる"),
            ("ko", "닫기"),
            ("ru", "Закрыть"),
            ("de", "Schließen"),
            ("fr", "Fermer"),
            ("es", "Cerrar"),
            ("pt-BR", "Fechar"),
            ("it", "Chiudi"),
            ("tr", "Kapat"),
            ("vi", "Đóng"),
            ("pl", "Zamknij"),
            ("uk", "Закрити"),
            ("id", "Tutup"),
        ],
    ),
    (
        "bern.window.minimize",
        &[
            ("zh-Hans", "最小化"),
            ("zh-Hant", "最小化"),
            ("en", "Minimize"),
            ("ja", "しまう"),
            ("ko", "최소화"),
            ("ru", "Свернуть"),
            ("de", "Minimieren"),
            ("fr", "Réduire"),
            ("es", "Minimizar"),
            ("pt-BR", "Minimizar"),
            ("it", "Riduci a icona"),
            ("tr", "Küçült"),
            ("vi", "Thu nhỏ"),
            ("pl", "Minimalizuj"),
            ("uk", "Згорнути"),
            ("id", "Minimalkan"),
        ],
    ),
    (
        "bern.window.maximize",
        &[
            ("zh-Hans", "最大化"),
            ("zh-Hant", "最大化"),
            ("en", "Maximize"),
            ("ja", "拡大"),
            ("ko", "최대화"),
            ("ru", "Развернуть"),
            ("de", "Maximieren"),
            ("fr", "Agrandir"),
            ("es", "Maximizar"),
            ("pt-BR", "Maximizar"),
            ("it", "Ingrandisci"),
            ("tr", "Büyüt"),
            ("vi", "Phóng to"),
            ("pl", "Maksymalizuj"),
            ("uk", "Розгорнути"),
            ("id", "Maksimalkan"),
        ],
    ),
    (
        "bern.window.restore",
        &[
            ("zh-Hans", "恢复"),
            ("zh-Hant", "恢復"),
            ("en", "Restore"),
            ("ja", "元のサイズに戻す"),
            ("ko", "복원"),
            ("ru", "Восстановить"),
            ("de", "Wiederherstellen"),
            ("fr", "Restaurer"),
            ("es", "Restaurar"),
            ("pt-BR", "Restaurar"),
            ("it", "Ripristina"),
            ("tr", "Geri yükle"),
            ("vi", "Khôi phục"),
            ("pl", "Przywróć"),
            ("uk", "Відновити"),
            ("id", "Pulihkan"),
        ],
    ),
    (
        "bern.window.switcher",
        &[
            ("zh-Hans", "窗口"),
            ("zh-Hant", "視窗"),
            ("en", "Windows"),
            ("ja", "ウインドウ"),
            ("ko", "창"),
            ("ru", "Окна"),
            ("de", "Fenster"),
            ("fr", "Fenêtres"),
            ("es", "Ventanas"),
            ("pt-BR", "Janelas"),
            ("it", "Finestre"),
            ("tr", "Pencereler"),
            ("vi", "Cửa sổ"),
            ("pl", "Okna"),
            ("uk", "Вікна"),
            ("id", "Jendela"),
        ],
    ),
    (
        "bern.window.reopen",
        &[
            ("zh-Hans", "打开“{title}”"),
            ("zh-Hant", "打開「{title}」"),
            ("en", "Open {title}"),
            ("ja", "「{title}」を開く"),
            ("ko", "{title} 열기"),
            ("ru", "Открыть {title}"),
            ("de", "{title} öffnen"),
            ("fr", "Ouvrir {title}"),
            ("es", "Abrir {title}"),
            ("pt-BR", "Abrir {title}"),
            ("it", "Apri {title}"),
            ("tr", "{title} aç"),
            ("vi", "Mở {title}"),
            ("pl", "Otwórz {title}"),
            ("uk", "Відкрити {title}"),
            ("id", "Buka {title}"),
        ],
    ),
    (
        "bern.window.settings",
        &[
            ("zh-Hans", "设置"),
            ("zh-Hant", "設定"),
            ("en", "Settings"),
            ("ja", "設定"),
            ("ko", "설정"),
            ("ru", "Настройки"),
            ("de", "Einstellungen"),
            ("fr", "Réglages"),
            ("es", "Ajustes"),
            ("pt-BR", "Configurações"),
            ("it", "Impostazioni"),
            ("tr", "Ayarlar"),
            ("vi", "Cài đặt"),
            ("pl", "Ustawienia"),
            ("uk", "Налаштування"),
            ("id", "Pengaturan"),
        ],
    ),
    (
        "bern.alert.failed",
        &[
            ("zh-Hans", "无法完成操作"),
            ("zh-Hant", "無法完成操作"),
            ("en", "The action couldn’t be completed"),
            ("ja", "操作を完了できませんでした"),
            ("ko", "작업을 완료할 수 없습니다"),
            ("ru", "Не удалось выполнить действие"),
            ("de", "Die Aktion konnte nicht abgeschlossen werden"),
            ("fr", "L’action n’a pas pu être effectuée"),
            ("es", "No se pudo completar la acción"),
            ("pt-BR", "Não foi possível concluir a ação"),
            ("it", "Impossibile completare l’azione"),
            ("tr", "İşlem tamamlanamadı"),
            ("vi", "Không thể hoàn tất thao tác"),
            ("pl", "Nie udało się wykonać działania"),
            ("uk", "Не вдалося виконати дію"),
            ("id", "Tindakan tidak dapat diselesaikan"),
        ],
    ),
    (
        "bern.alert.ok",
        &[
            ("zh-Hans", "好"),
            ("zh-Hant", "好"),
            ("en", "OK"),
            ("ja", "OK"),
            ("ko", "확인"),
            ("ru", "ОК"),
            ("de", "OK"),
            ("fr", "OK"),
            ("es", "Aceptar"),
            ("pt-BR", "OK"),
            ("it", "OK"),
            ("tr", "Tamam"),
            ("vi", "OK"),
            ("pl", "OK"),
            ("uk", "Гаразд"),
            ("id", "OK"),
        ],
    ),
    (
        "bern.startup.failed",
        &[
            ("zh-Hans", "应用未能启动"),
            ("zh-Hant", "App 無法啟動"),
            ("en", "The app couldn’t start"),
            ("ja", "アプリを起動できませんでした"),
            ("ko", "앱을 시작할 수 없습니다"),
            ("ru", "Не удалось запустить приложение"),
            ("de", "Die App konnte nicht gestartet werden"),
            ("fr", "L’application n’a pas pu démarrer"),
            ("es", "No se pudo iniciar la aplicación"),
            ("pt-BR", "Não foi possível iniciar o aplicativo"),
            ("it", "Impossibile avviare l’app"),
            ("tr", "Uygulama başlatılamadı"),
            ("vi", "Không thể khởi động ứng dụng"),
            ("pl", "Nie udało się uruchomić aplikacji"),
            ("uk", "Не вдалося запустити застосунок"),
            ("id", "Aplikasi tidak dapat dimulai"),
        ],
    ),
    (
        "bern.split.more",
        &[
            ("zh-Hans", "更多选项"),
            ("zh-Hant", "更多選項"),
            ("en", "More Options"),
            ("ja", "その他のオプション"),
            ("ko", "추가 옵션"),
            ("ru", "Другие параметры"),
            ("de", "Weitere Optionen"),
            ("fr", "Plus d’options"),
            ("es", "Más opciones"),
            ("pt-BR", "Mais opções"),
            ("it", "Altre opzioni"),
            ("tr", "Diğer seçenekler"),
            ("vi", "Tùy chọn khác"),
            ("pl", "Więcej opcji"),
            ("uk", "Інші параметри"),
            ("id", "Opsi lainnya"),
        ],
    ),
    (
        "bern.query.loading",
        &[
            ("zh-Hans", "正在载入…"),
            ("zh-Hant", "正在載入…"),
            ("en", "Loading…"),
            ("ja", "読み込み中…"),
            ("ko", "불러오는 중…"),
            ("ru", "Загрузка…"),
            ("de", "Wird geladen…"),
            ("fr", "Chargement…"),
            ("es", "Cargando…"),
            ("pt-BR", "Carregando…"),
            ("it", "Caricamento…"),
            ("tr", "Yükleniyor…"),
            ("vi", "Đang tải…"),
            ("pl", "Wczytywanie…"),
            ("uk", "Завантаження…"),
            ("id", "Memuat…"),
        ],
    ),
    (
        "bern.query.failed",
        &[
            ("zh-Hans", "无法载入"),
            ("zh-Hant", "無法載入"),
            ("en", "Couldn’t Load"),
            ("ja", "読み込めませんでした"),
            ("ko", "불러올 수 없음"),
            ("ru", "Не удалось загрузить"),
            ("de", "Laden fehlgeschlagen"),
            ("fr", "Chargement impossible"),
            ("es", "No se pudo cargar"),
            ("pt-BR", "Não foi possível carregar"),
            ("it", "Impossibile caricare"),
            ("tr", "Yüklenemedi"),
            ("vi", "Không thể tải"),
            ("pl", "Nie udało się wczytać"),
            ("uk", "Не вдалося завантажити"),
            ("id", "Tidak dapat memuat"),
        ],
    ),
    (
        "bern.query.retry",
        &[
            ("zh-Hans", "重试"),
            ("zh-Hant", "重試"),
            ("en", "Try Again"),
            ("ja", "再試行"),
            ("ko", "다시 시도"),
            ("ru", "Повторить"),
            ("de", "Erneut versuchen"),
            ("fr", "Réessayer"),
            ("es", "Reintentar"),
            ("pt-BR", "Tentar novamente"),
            ("it", "Riprova"),
            ("tr", "Yeniden dene"),
            ("vi", "Thử lại"),
            ("pl", "Spróbuj ponownie"),
            ("uk", "Спробувати ще раз"),
            ("id", "Coba lagi"),
        ],
    ),
    (
        "bern.media.video",
        &[
            ("zh-Hans", "视频"),
            ("zh-Hant", "影片"),
            ("en", "Video"),
            ("ja", "ビデオ"),
            ("ko", "동영상"),
            ("ru", "Видео"),
            ("de", "Video"),
            ("fr", "Vidéo"),
            ("es", "Vídeo"),
            ("pt-BR", "Vídeo"),
            ("it", "Video"),
            ("tr", "Video"),
            ("vi", "Video"),
            ("pl", "Wideo"),
            ("uk", "Відео"),
            ("id", "Video"),
        ],
    ),
    (
        "bern.media.play",
        &[
            ("zh-Hans", "播放"),
            ("zh-Hant", "播放"),
            ("en", "Play"),
            ("ja", "再生"),
            ("ko", "재생"),
            ("ru", "Воспроизвести"),
            ("de", "Wiedergeben"),
            ("fr", "Lire"),
            ("es", "Reproducir"),
            ("pt-BR", "Reproduzir"),
            ("it", "Riproduci"),
            ("tr", "Oynat"),
            ("vi", "Phát"),
            ("pl", "Odtwórz"),
            ("uk", "Відтворити"),
            ("id", "Putar"),
        ],
    ),
    (
        "bern.media.pause",
        &[
            ("zh-Hans", "暂停"),
            ("zh-Hant", "暫停"),
            ("en", "Pause"),
            ("ja", "一時停止"),
            ("ko", "일시 정지"),
            ("ru", "Пауза"),
            ("de", "Pause"),
            ("fr", "Pause"),
            ("es", "Pausa"),
            ("pt-BR", "Pausar"),
            ("it", "Pausa"),
            ("tr", "Duraklat"),
            ("vi", "Tạm dừng"),
            ("pl", "Pauza"),
            ("uk", "Пауза"),
            ("id", "Jeda"),
        ],
    ),
    (
        "bern.media.mute",
        &[
            ("zh-Hans", "静音"),
            ("zh-Hant", "靜音"),
            ("en", "Mute"),
            ("ja", "消音"),
            ("ko", "음소거"),
            ("ru", "Выключить звук"),
            ("de", "Stummschalten"),
            ("fr", "Couper le son"),
            ("es", "Silenciar"),
            ("pt-BR", "Silenciar"),
            ("it", "Disattiva audio"),
            ("tr", "Sesi kapat"),
            ("vi", "Tắt tiếng"),
            ("pl", "Wycisz"),
            ("uk", "Вимкнути звук"),
            ("id", "Bisukan"),
        ],
    ),
    (
        "bern.media.unmute",
        &[
            ("zh-Hans", "取消静音"),
            ("zh-Hant", "取消靜音"),
            ("en", "Unmute"),
            ("ja", "消音を解除"),
            ("ko", "음소거 해제"),
            ("ru", "Включить звук"),
            ("de", "Ton einschalten"),
            ("fr", "Rétablir le son"),
            ("es", "Activar sonido"),
            ("pt-BR", "Ativar som"),
            ("it", "Attiva audio"),
            ("tr", "Sesi aç"),
            ("vi", "Bật tiếng"),
            ("pl", "Włącz dźwięk"),
            ("uk", "Увімкнути звук"),
            ("id", "Aktifkan suara"),
        ],
    ),
    (
        "bern.media.position",
        &[
            ("zh-Hans", "播放位置"),
            ("zh-Hant", "播放位置"),
            ("en", "Playback Position"),
            ("ja", "再生位置"),
            ("ko", "재생 위치"),
            ("ru", "Позиция воспроизведения"),
            ("de", "Wiedergabeposition"),
            ("fr", "Position de lecture"),
            ("es", "Posición de reproducción"),
            ("pt-BR", "Posição de reprodução"),
            ("it", "Posizione di riproduzione"),
            ("tr", "Oynatma konumu"),
            ("vi", "Vị trí phát"),
            ("pl", "Pozycja odtwarzania"),
            ("uk", "Позиція відтворення"),
            ("id", "Posisi pemutaran"),
        ],
    ),
    (
        "bern.media.volume",
        &[
            ("zh-Hans", "音量"),
            ("zh-Hant", "音量"),
            ("en", "Volume"),
            ("ja", "音量"),
            ("ko", "음량"),
            ("ru", "Громкость"),
            ("de", "Lautstärke"),
            ("fr", "Volume"),
            ("es", "Volumen"),
            ("pt-BR", "Volume"),
            ("it", "Volume"),
            ("tr", "Ses düzeyi"),
            ("vi", "Âm lượng"),
            ("pl", "Głośność"),
            ("uk", "Гучність"),
            ("id", "Volume"),
        ],
    ),
    (
        "bern.media.live",
        &[
            ("zh-Hans", "直播"),
            ("zh-Hant", "直播"),
            ("en", "Live"),
            ("ja", "ライブ"),
            ("ko", "실시간"),
            ("ru", "Прямой эфир"),
            ("de", "Live"),
            ("fr", "En direct"),
            ("es", "En directo"),
            ("pt-BR", "Ao vivo"),
            ("it", "In diretta"),
            ("tr", "Canlı"),
            ("vi", "Trực tiếp"),
            ("pl", "Na żywo"),
            ("uk", "Наживо"),
            ("id", "Langsung"),
        ],
    ),
    (
        "bern.media.empty",
        &[
            ("zh-Hans", "没有视频"),
            ("zh-Hant", "沒有影片"),
            ("en", "No Video"),
            ("ja", "ビデオがありません"),
            ("ko", "동영상 없음"),
            ("ru", "Нет видео"),
            ("de", "Kein Video"),
            ("fr", "Aucune vidéo"),
            ("es", "Sin vídeo"),
            ("pt-BR", "Nenhum vídeo"),
            ("it", "Nessun video"),
            ("tr", "Video yok"),
            ("vi", "Không có video"),
            ("pl", "Brak wideo"),
            ("uk", "Немає відео"),
            ("id", "Tidak ada video"),
        ],
    ),
    (
        "bern.media.failed",
        &[
            ("zh-Hans", "无法播放"),
            ("zh-Hant", "無法播放"),
            ("en", "Can’t Play"),
            ("ja", "再生できません"),
            ("ko", "재생할 수 없음"),
            ("ru", "Не удалось воспроизвести"),
            ("de", "Wiedergabe nicht möglich"),
            ("fr", "Lecture impossible"),
            ("es", "No se puede reproducir"),
            ("pt-BR", "Não foi possível reproduzir"),
            ("it", "Impossibile riprodurre"),
            ("tr", "Oynatılamıyor"),
            ("vi", "Không thể phát"),
            ("pl", "Nie można odtworzyć"),
            ("uk", "Не вдалося відтворити"),
            ("id", "Tidak dapat memutar"),
        ],
    ),
    (
        "bern.media.unavailable",
        &[
            ("zh-Hans", "播放器不可用"),
            ("zh-Hant", "播放器無法使用"),
            ("en", "Player Unavailable"),
            ("ja", "プレーヤーを使用できません"),
            ("ko", "플레이어를 사용할 수 없음"),
            ("ru", "Плеер недоступен"),
            ("de", "Player nicht verfügbar"),
            ("fr", "Lecteur indisponible"),
            ("es", "Reproductor no disponible"),
            ("pt-BR", "Reprodutor indisponível"),
            ("it", "Lettore non disponibile"),
            ("tr", "Oynatıcı kullanılamıyor"),
            ("vi", "Trình phát không khả dụng"),
            ("pl", "Odtwarzacz niedostępny"),
            ("uk", "Програвач недоступний"),
            ("id", "Pemutar tidak tersedia"),
        ],
    ),
    (
        "bern.close.save",
        &[
            ("zh-Hans", "存储"),
            ("zh-Hant", "儲存"),
            ("en", "Save"),
            ("ja", "保存"),
            ("ko", "저장"),
            ("ru", "Сохранить"),
            ("de", "Sichern"),
            ("fr", "Enregistrer"),
            ("es", "Guardar"),
            ("pt-BR", "Salvar"),
            ("it", "Salva"),
            ("tr", "Kaydet"),
            ("vi", "Lưu"),
            ("pl", "Zapisz"),
            ("uk", "Зберегти"),
            ("id", "Simpan"),
        ],
    ),
    (
        "bern.close.dont_save",
        &[
            ("zh-Hans", "不存储"),
            ("zh-Hant", "不儲存"),
            ("en", "Don’t Save"),
            ("ja", "保存しない"),
            ("ko", "저장 안 함"),
            ("ru", "Не сохранять"),
            ("de", "Nicht sichern"),
            ("fr", "Ne pas enregistrer"),
            ("es", "No guardar"),
            ("pt-BR", "Não salvar"),
            ("it", "Non salvare"),
            ("tr", "Kaydetme"),
            ("vi", "Không lưu"),
            ("pl", "Nie zapisuj"),
            ("uk", "Не зберігати"),
            ("id", "Jangan simpan"),
        ],
    ),
    (
        "bern.close.discard",
        &[
            ("zh-Hans", "放弃更改"),
            ("zh-Hant", "放棄變更"),
            ("en", "Discard Changes"),
            ("ja", "変更を破棄"),
            ("ko", "변경 사항 버리기"),
            ("ru", "Отменить изменения"),
            ("de", "Änderungen verwerfen"),
            ("fr", "Annuler les modifications"),
            ("es", "Descartar cambios"),
            ("pt-BR", "Descartar alterações"),
            ("it", "Annulla modifiche"),
            ("tr", "Değişiklikleri sil"),
            ("vi", "Hủy thay đổi"),
            ("pl", "Odrzuć zmiany"),
            ("uk", "Відкинути зміни"),
            ("id", "Buang perubahan"),
        ],
    ),
    (
        "bern.close.cancel",
        &[
            ("zh-Hans", "取消"),
            ("zh-Hant", "取消"),
            ("en", "Cancel"),
            ("ja", "キャンセル"),
            ("ko", "취소"),
            ("ru", "Отмена"),
            ("de", "Abbrechen"),
            ("fr", "Annuler"),
            ("es", "Cancelar"),
            ("pt-BR", "Cancelar"),
            ("it", "Annulla"),
            ("tr", "İptal"),
            ("vi", "Hủy"),
            ("pl", "Anuluj"),
            ("uk", "Скасувати"),
            ("id", "Batal"),
        ],
    ),
    (
        "bern.close.save_detail",
        &[
            ("zh-Hans", "如果不存储，你的更改将会丢失。"),
            ("zh-Hant", "如果不儲存，你的變更將會遺失。"),
            ("en", "Your changes will be lost if you don’t save them."),
            ("ja", "保存しないと、変更は失われます。"),
            ("ko", "저장하지 않으면 변경 사항이 사라집니다."),
            ("ru", "Несохранённые изменения будут потеряны."),
            ("de", "Nicht gesicherte Änderungen gehen verloren."),
            ("fr", "Les modifications non enregistrées seront perdues."),
            ("es", "Los cambios se perderán si no los guardas."),
            ("pt-BR", "As alterações serão perdidas se não forem salvas."),
            ("it", "Le modifiche non salvate andranno perse."),
            ("tr", "Kaydetmezseniz değişiklikler kaybolacak."),
            ("vi", "Các thay đổi sẽ bị mất nếu bạn không lưu."),
            ("pl", "Niezapisane zmiany zostaną utracone."),
            ("uk", "Незбережені зміни буде втрачено."),
            ("id", "Perubahan akan hilang jika tidak disimpan."),
        ],
    ),
    (
        "bern.close.discard_detail",
        &[
            ("zh-Hans", "关闭窗口会放弃这些更改。"),
            ("zh-Hant", "關閉視窗會放棄這些變更。"),
            ("en", "Closing the window discards these changes."),
            ("ja", "ウインドウを閉じると、これらの変更は破棄されます。"),
            ("ko", "창을 닫으면 변경 사항이 사라집니다."),
            ("ru", "Закрытие окна отменит эти изменения."),
            (
                "de",
                "Beim Schließen des Fensters werden diese Änderungen verworfen.",
            ),
            ("fr", "Fermer la fenêtre annule ces modifications."),
            ("es", "Al cerrar la ventana se descartan estos cambios."),
            ("pt-BR", "Fechar a janela descarta estas alterações."),
            (
                "it",
                "Chiudendo la finestra, queste modifiche verranno annullate.",
            ),
            ("tr", "Pencere kapatıldığında bu değişiklikler silinir."),
            ("vi", "Đóng cửa sổ sẽ hủy các thay đổi này."),
            ("pl", "Zamknięcie okna odrzuci te zmiany."),
            ("uk", "Закриття вікна скасує ці зміни."),
            ("id", "Menutup jendela akan membuang perubahan ini."),
        ],
    ),
    (
        "bern.navigation.back",
        &[
            ("zh-Hans", "返回"),
            ("zh-Hant", "返回"),
            ("en", "Back"),
            ("ja", "戻る"),
            ("ko", "뒤로"),
            ("ru", "Назад"),
            ("de", "Zurück"),
            ("fr", "Retour"),
            ("es", "Atrás"),
            ("pt-BR", "Voltar"),
            ("it", "Indietro"),
            ("tr", "Geri"),
            ("vi", "Quay lại"),
            ("pl", "Wstecz"),
            ("uk", "Назад"),
            ("id", "Kembali"),
        ],
    ),
    (
        "bern.navigation.sidebar",
        &[
            ("zh-Hans", "边栏"),
            ("zh-Hant", "側邊欄"),
            ("en", "Sidebar"),
            ("ja", "サイドバー"),
            ("ko", "사이드바"),
            ("ru", "Боковая панель"),
            ("de", "Seitenleiste"),
            ("fr", "Barre latérale"),
            ("es", "Barra lateral"),
            ("pt-BR", "Barra lateral"),
            ("it", "Barra laterale"),
            ("tr", "Kenar çubuğu"),
            ("vi", "Thanh bên"),
            ("pl", "Panel boczny"),
            ("uk", "Бічна панель"),
            ("id", "Bilah samping"),
        ],
    ),
    (
        "bern.navigation.hide_sidebar",
        &[
            ("zh-Hans", "隐藏边栏"),
            ("zh-Hant", "隱藏側邊欄"),
            ("en", "Hide Sidebar"),
            ("ja", "サイドバーを非表示"),
            ("ko", "사이드바 숨기기"),
            ("ru", "Скрыть боковую панель"),
            ("de", "Seitenleiste ausblenden"),
            ("fr", "Masquer la barre latérale"),
            ("es", "Ocultar barra lateral"),
            ("pt-BR", "Ocultar barra lateral"),
            ("it", "Nascondi barra laterale"),
            ("tr", "Kenar çubuğunu gizle"),
            ("vi", "Ẩn thanh bên"),
            ("pl", "Ukryj panel boczny"),
            ("uk", "Приховати бічну панель"),
            ("id", "Sembunyikan bilah samping"),
        ],
    ),
    (
        "bern.navigation.show_sidebar",
        &[
            ("zh-Hans", "显示边栏"),
            ("zh-Hant", "顯示側邊欄"),
            ("en", "Show Sidebar"),
            ("ja", "サイドバーを表示"),
            ("ko", "사이드바 표시"),
            ("ru", "Показать боковую панель"),
            ("de", "Seitenleiste einblenden"),
            ("fr", "Afficher la barre latérale"),
            ("es", "Mostrar barra lateral"),
            ("pt-BR", "Mostrar barra lateral"),
            ("it", "Mostra barra laterale"),
            ("tr", "Kenar çubuğunu göster"),
            ("vi", "Hiện thanh bên"),
            ("pl", "Pokaż panel boczny"),
            ("uk", "Показати бічну панель"),
            ("id", "Tampilkan bilah samping"),
        ],
    ),
    (
        "bern.runtime.missing",
        &[
            (
                "zh-Hans",
                "此应用需要 Windows App Runtime 2，这台电脑上还没有。\n\n要现在用浏览器下载微软的安装程序吗？安装完成后请重新打开此应用。",
            ),
            (
                "zh-Hant",
                "此 App 需要 Windows App Runtime 2，這台電腦上還沒有。\n\n要現在以瀏覽器下載 Microsoft 的安裝程式嗎？安裝完成後請重新開啟此 App。",
            ),
            (
                "en",
                "This app needs the Windows App Runtime 2, which this computer doesn’t have yet.\n\nDownload Microsoft’s installer in your browser now? Open the app again once it’s installed.",
            ),
            (
                "ja",
                "このアプリには Windows App Runtime 2 が必要ですが、このコンピューターにはまだありません。\n\nMicrosoft のインストーラーを今すぐブラウザーでダウンロードしますか？インストール後にアプリをもう一度開いてください。",
            ),
            (
                "ko",
                "이 앱에는 Windows App Runtime 2가 필요합니다. 브라우저에서 Microsoft 설치 프로그램을 다운로드하시겠습니까? 설치 후 앱을 다시 열어 주세요.",
            ),
            (
                "ru",
                "Приложению требуется Windows App Runtime 2. Скачать установщик Microsoft в браузере? После установки откройте приложение снова.",
            ),
            (
                "de",
                "Diese App benötigt Windows App Runtime 2. Das Installationsprogramm von Microsoft jetzt im Browser herunterladen? Öffne die App nach der Installation erneut.",
            ),
            (
                "fr",
                "Cette application nécessite Windows App Runtime 2. Télécharger le programme d’installation de Microsoft dans le navigateur ? Rouvrez l’application après l’installation.",
            ),
            (
                "es",
                "Esta aplicación necesita Windows App Runtime 2. ¿Descargar el instalador de Microsoft en el navegador? Vuelve a abrir la aplicación después de instalarlo.",
            ),
            (
                "pt-BR",
                "Este aplicativo precisa do Windows App Runtime 2. Baixar o instalador da Microsoft no navegador? Abra o aplicativo novamente após a instalação.",
            ),
            (
                "it",
                "Questa app richiede Windows App Runtime 2. Scaricare il programma di installazione Microsoft nel browser? Riapri l’app dopo l’installazione.",
            ),
            (
                "tr",
                "Bu uygulama Windows App Runtime 2 gerektirir. Microsoft yükleyicisi tarayıcıda indirilsin mi? Yükledikten sonra uygulamayı yeniden açın.",
            ),
            (
                "vi",
                "Ứng dụng này cần Windows App Runtime 2. Tải trình cài đặt của Microsoft trong trình duyệt? Mở lại ứng dụng sau khi cài đặt.",
            ),
            (
                "pl",
                "Ta aplikacja wymaga Windows App Runtime 2. Pobrać instalator Microsoft w przeglądarce? Po instalacji otwórz aplikację ponownie.",
            ),
            (
                "uk",
                "Цьому застосунку потрібен Windows App Runtime 2. Завантажити інсталятор Microsoft у браузері? Після встановлення знову відкрийте застосунок.",
            ),
            (
                "id",
                "Aplikasi ini memerlukan Windows App Runtime 2. Unduh penginstal Microsoft di browser sekarang? Buka kembali aplikasi setelah selesai menginstal.",
            ),
        ],
    ),
    (
        "bern.search.clear",
        &[
            ("zh-Hans", "清除"),
            ("zh-Hant", "清除"),
            ("en", "Clear"),
            ("ja", "消去"),
            ("ko", "지우기"),
            ("ru", "Очистить"),
            ("de", "Löschen"),
            ("fr", "Effacer"),
            ("es", "Borrar"),
            ("pt-BR", "Limpar"),
            ("it", "Cancella"),
            ("tr", "Temizle"),
            ("vi", "Xóa"),
            ("pl", "Wyczyść"),
            ("uk", "Очистити"),
            ("id", "Hapus"),
        ],
    ),
    (
        "bern.link.lost",
        &[
            ("zh-Hans", "与引擎的连接已断开，正在重新连接…"),
            ("zh-Hant", "與引擎的連線已中斷，正在重新連線…"),
            ("en", "Lost the connection to the engine. Reconnecting…"),
            ("ja", "エンジンとの接続が切れました。再接続しています…"),
            ("ko", "엔진과 연결이 끊겼습니다. 다시 연결하는 중…"),
            ("ru", "Связь с движком потеряна. Повторное подключение…"),
            (
                "de",
                "Verbindung zur Engine verloren. Verbindung wird wiederhergestellt…",
            ),
            ("fr", "Connexion au moteur perdue. Reconnexion…"),
            ("es", "Se perdió la conexión con el motor. Reconectando…"),
            (
                "pt-BR",
                "A conexão com o mecanismo foi perdida. Reconectando…",
            ),
            ("it", "Connessione al motore persa. Riconnessione…"),
            ("tr", "Motor bağlantısı kesildi. Yeniden bağlanılıyor…"),
            ("vi", "Mất kết nối với bộ máy. Đang kết nối lại…"),
            ("pl", "Utracono połączenie z silnikiem. Ponowne łączenie…"),
            ("uk", "Зв’язок із рушієм втрачено. Повторне підключення…"),
            ("id", "Koneksi ke mesin terputus. Menghubungkan kembali…"),
        ],
    ),
    (
        "bern.link.restarted",
        &[
            ("zh-Hans", "引擎已重新启动，页面将重新载入。"),
            ("zh-Hant", "引擎已重新啟動，頁面將重新載入。"),
            ("en", "The engine restarted. Reloading the page."),
            ("ja", "エンジンが再起動しました。ページを再読み込みします。"),
            ("ko", "엔진이 다시 시작되었습니다. 페이지를 새로 고칩니다."),
            ("ru", "Движок перезапущен. Перезагрузка страницы."),
            (
                "de",
                "Die Engine wurde neu gestartet. Die Seite wird neu geladen.",
            ),
            ("fr", "Le moteur a redémarré. Rechargement de la page."),
            ("es", "El motor se reinició. Recargando la página."),
            (
                "pt-BR",
                "O mecanismo foi reiniciado. Recarregando a página.",
            ),
            (
                "it",
                "Il motore è stato riavviato. Ricaricamento della pagina.",
            ),
            ("tr", "Motor yeniden başlatıldı. Sayfa yeniden yükleniyor."),
            ("vi", "Bộ máy đã khởi động lại. Đang tải lại trang."),
            (
                "pl",
                "Silnik został uruchomiony ponownie. Ponowne wczytywanie strony.",
            ),
            ("uk", "Рушій перезапущено. Перезавантаження сторінки."),
            ("id", "Mesin dimulai ulang. Memuat ulang halaman."),
        ],
    ),
    (
        "bern.link.failed",
        &[
            ("zh-Hans", "无法连接到引擎"),
            ("zh-Hant", "無法連線到引擎"),
            ("en", "Can’t Connect to the Engine"),
            ("ja", "エンジンに接続できません"),
            ("ko", "엔진에 연결할 수 없음"),
            ("ru", "Не удалось подключиться к движку"),
            ("de", "Verbindung zur Engine nicht möglich"),
            ("fr", "Connexion au moteur impossible"),
            ("es", "No se puede conectar con el motor"),
            ("pt-BR", "Não foi possível conectar ao mecanismo"),
            ("it", "Impossibile connettersi al motore"),
            ("tr", "Motora bağlanılamıyor"),
            ("vi", "Không thể kết nối với bộ máy"),
            ("pl", "Nie można połączyć się z silnikiem"),
            ("uk", "Не вдалося підключитися до рушія"),
            ("id", "Tidak dapat terhubung ke mesin"),
        ],
    ),
    // How a `ShortcutInput` and a menu write the space bar, and what a
    // `ShortcutInput` shows while it waits for a key and without a shortcut.
    (
        "bern.shortcut.space",
        &[
            ("zh-Hans", "空格"),
            ("zh-Hant", "空白鍵"),
            ("en", "Space"),
            ("ja", "スペース"),
            ("ko", "스페이스"),
            ("ru", "Пробел"),
            ("de", "Leertaste"),
            ("fr", "Espace"),
            ("es", "Espacio"),
            ("pt-BR", "Espaço"),
            ("it", "Spazio"),
            ("tr", "Boşluk"),
            ("vi", "Dấu cách"),
            ("pl", "Spacja"),
            ("uk", "Пробіл"),
            ("id", "Spasi"),
        ],
    ),
    (
        "bern.shortcut.prompt",
        &[
            ("zh-Hans", "按下快捷键…"),
            ("zh-Hant", "按下快速鍵…"),
            ("en", "Press a Shortcut…"),
            ("ja", "ショートカットを押してください…"),
            ("ko", "단축키를 누르세요…"),
            ("ru", "Нажмите сочетание…"),
            ("de", "Kurzbefehl drücken …"),
            ("fr", "Appuyez sur un raccourci…"),
            ("es", "Pulsa un atajo…"),
            ("pt-BR", "Pressione um atalho…"),
            ("it", "Premi una scorciatoia…"),
            ("tr", "Bir kısayola basın…"),
            ("vi", "Nhấn phím tắt…"),
            ("pl", "Naciśnij skrót…"),
            ("uk", "Натисніть сполучення…"),
            ("id", "Tekan pintasan…"),
        ],
    ),
    (
        "bern.shortcut.none",
        &[
            ("zh-Hans", "无"),
            ("zh-Hant", "無"),
            ("en", "None"),
            ("ja", "なし"),
            ("ko", "없음"),
            ("ru", "Нет"),
            ("de", "Keiner"),
            ("fr", "Aucun"),
            ("es", "Ninguno"),
            ("pt-BR", "Nenhum"),
            ("it", "Nessuno"),
            ("tr", "Yok"),
            ("vi", "Không có"),
            ("pl", "Brak"),
            ("uk", "Немає"),
            ("id", "Tidak ada"),
        ],
    ),
];

/// Languages the runtime's own strings are translated into.
pub fn framework_languages() -> Vec<&'static str> {
    FRAMEWORK_TEXTS[0]
        .1
        .iter()
        .map(|(language, _)| *language)
        .collect()
}

/// Convert Bern placeholders to positional format specifiers made by
/// `specifier` from the 1-based position, escaping `%` as `%%`. Texts
/// without parameters are not formatted, so only their braces are
/// unescaped.
pub fn positional(text: &str, params: &[String], specifier: impl Fn(usize) -> String) -> String {
    if params.is_empty() {
        return unescape_braces(text);
    }
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push('}');
            }
            '{' => {
                let mut name = String::new();
                for next in chars.by_ref() {
                    if next == '}' {
                        break;
                    }
                    name.push(next);
                }
                let position = params.iter().position(|param| *param == name).unwrap_or(0) + 1;
                out.push_str(&specifier(position));
            }
            '%' => out.push_str("%%"),
            c => out.push(c),
        }
    }
    out
}

/// The text in `language`, else in the default language, else any.
pub fn translation<'a>(text: &'a LocalizedText, language: &str, default: &str) -> &'a str {
    text.get(language)
        .or_else(|| text.get(default))
        .or_else(|| text.translations.values().next().map(String::as_str))
        .unwrap_or("")
}

/// A framework text in `language`, else in English.
pub fn framework(translations: &[(&str, &'static str)], language: &str) -> &'static str {
    translations
        .iter()
        .find(|(candidate, _)| *candidate == language)
        .or_else(|| {
            translations
                .iter()
                .find(|(candidate, _)| *candidate == "en")
        })
        .map(|(_, value)| *value)
        .unwrap_or("")
}

/// A text without parameters as it is shown: `{{` and `}}` stand for one
/// brace, and nothing is filled in at run time.
pub fn unescape_braces(text: &str) -> String {
    text.replace("{{", "{").replace("}}", "}")
}

/// `(language, [(key, text)])` for every supported language, for the
/// runtimes that fill Bern's placeholders by name: the declared texts, the
/// application's name as `app.name`, the framework texts, and the weekday
/// and month names, which fall back to English in a language Bern has none
/// for. Texts with parameters keep their format and escapes for the
/// runtime; the others are stored as they are shown.
pub fn tables(app: &Application) -> Vec<(String, Vec<(String, String)>)> {
    let default = &app.languages.default;
    app.languages
        .supported
        .iter()
        .map(|language| {
            let mut table: Vec<(String, String)> = app
                .texts
                .iter()
                .map(|text| {
                    let value = translation(text, language, default);
                    let value = if text.params.is_empty() {
                        unescape_braces(value)
                    } else {
                        value.to_string()
                    };
                    (text.key.clone(), value)
                })
                .collect();
            let name = app
                .app
                .name
                .get(language)
                .or_else(|| app.app.name.get(default))
                .unwrap_or(&app.app.id);
            table.push(("app.name".into(), name.to_string()));
            for (key, translations) in FRAMEWORK_TEXTS {
                table.push(((*key).into(), framework(translations, language).into()));
            }
            for (key, translations) in bern_protocol::format::date_names() {
                let value = translations
                    .iter()
                    .find(|(candidate, _)| candidate == language)
                    .or_else(|| {
                        translations
                            .iter()
                            .find(|(candidate, _)| *candidate == "en")
                    })
                    .map(|(_, value)| value.clone())
                    .unwrap_or_default();
                table.push((key, value));
            }
            (language.clone(), table)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bern_ir::{AppInfo, Languages, Theme, ThemeMode, WindowCatalog};
    use bern_protocol::{ProtocolDescriptor, TypeTable};
    use std::path::Path;

    fn text(key: &str, value: &str, params: &[&str]) -> LocalizedText {
        LocalizedText {
            key: key.into(),
            translations: [("en".to_string(), value.to_string())].into(),
            params: params.iter().map(|param| param.to_string()).collect(),
        }
    }

    fn app(texts: Vec<LocalizedText>) -> Application {
        Application {
            ir_version: bern_ir::IR_VERSION,
            app: AppInfo {
                id: "test".into(),
                bundle_id: "test.app".into(),
                name: LocalizedText::default(),
                version: "1".into(),
                icon: None,
                platform_icons: Default::default(),
                publisher: None,
            },
            languages: Languages {
                supported: vec!["en".into()],
                default: "en".into(),
                strict: true,
            },
            theme: Theme {
                mode: ThemeMode::System,
                brand_seed: None,
            },
            protocol: ProtocolDescriptor {
                descriptor_version: 1,
                namespace: "test".into(),
                version: 1,
                types: TypeTable::default(),
                state: vec![],
                commands: vec![],
                events: vec![],
                modules: vec![],
            },
            types: TypeTable::default(),
            sessions: vec![],
            windows: WindowCatalog {
                handle: "Windows".into(),
                primary: "main".into(),
                windows: vec![],
                accent: None,
                palette: None,
                appearance: None,
                menu_bar: None,
            },
            players: vec![],
            components: vec![],
            texts,
            resources: vec![],
            layouts: None,
            contracts: vec![],
            style: Default::default(),
        }
    }

    #[test]
    fn every_framework_text_has_every_framework_language_once() {
        let mut languages = framework_languages();
        languages.sort_unstable();
        let mut keys = Vec::new();
        for (key, translations) in FRAMEWORK_TEXTS {
            let mut found: Vec<_> = translations.iter().map(|(language, _)| *language).collect();
            found.sort_unstable();
            assert_eq!(found, languages, "{key}");
            assert!(!keys.contains(key), "{key} is defined twice");
            keys.push(*key);
        }
    }

    /// Every `"bern.…"` key the runtimes and generators show is defined, so
    /// a new runtime text cannot ship without its translations.
    #[test]
    fn every_text_a_runtime_shows_is_defined() {
        let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut dirs = vec![crates.join("bern-gtk/src")];
        for backend in ["swiftui", "compose", "gtk", "winui3", "web"] {
            let backend = crates.join(format!("bern-backend-{backend}"));
            dirs.push(backend.join("src"));
            dirs.push(backend.join("templates"));
        }
        let mut used = Vec::new();
        // Not every backend has templates of its own.
        while let Some(dir) = dirs.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    dirs.push(path);
                    continue;
                }
                let Ok(source) = std::fs::read_to_string(&path) else {
                    continue;
                };
                for (start, _) in source.match_indices("\"bern.") {
                    let key: String = source[start + 1..]
                        .chars()
                        .take_while(|c| c.is_ascii_lowercase() || *c == '_' || *c == '.')
                        .collect();
                    let quoted = source[start + 1 + key.len()..].starts_with(['"', '\\']);
                    if quoted && key.matches('.').count() >= 2 {
                        used.push((key, path.clone()));
                    }
                }
            }
        }
        assert!(
            used.len() > FRAMEWORK_TEXTS.len(),
            "the sources were not found"
        );
        let mut defined: Vec<String> = FRAMEWORK_TEXTS
            .iter()
            .map(|(key, _)| (*key).to_string())
            .collect();
        defined.extend(
            bern_protocol::format::date_names()
                .into_iter()
                .map(|(key, _)| key),
        );
        for (key, path) in used {
            // A key ending in `.` is the start of keys the runtime completes
            // as it runs, such as `bern.date.month.` and the month's number.
            let found = match key.strip_suffix('.') {
                Some(_) => defined.iter().any(|defined| defined.starts_with(&key)),
                None => defined.contains(&key),
            };
            assert!(
                found,
                "{} shows `{key}`, which neither FRAMEWORK_TEXTS nor the date names define",
                path.display()
            );
        }
    }

    #[test]
    fn tables_store_texts_without_parameters_as_shown() {
        let app = app(vec![
            text("plain", "Use {{x}} and }}", &[]),
            text("filled", "{n} of {{all}}", &["n"]),
        ]);
        let tables = tables(&app);
        assert_eq!(tables.len(), 1);
        let (language, table) = &tables[0];
        assert_eq!(language, "en");
        let get = |key: &str| {
            table
                .iter()
                .find(|(candidate, _)| candidate == key)
                .map(|(_, value)| value.as_str())
        };
        assert_eq!(get("plain"), Some("Use {x} and }"));
        // The runtime fills the placeholders and resolves the escapes.
        assert_eq!(get("filled"), Some("{n} of {{all}}"));
        // Without a translated name, the application id stands in.
        assert_eq!(get("app.name"), Some("test"));
        assert_eq!(get("bern.media.empty"), Some("No Video"));
        assert_eq!(
            table.len(),
            3 + FRAMEWORK_TEXTS.len() + bern_protocol::format::date_names().len()
        );
    }

    #[test]
    fn positional_specifiers_and_escapes() {
        let params = ["name".to_string(), "count".to_string()];
        assert_eq!(
            positional("{count} for {name}, 100% {{sure}}", &params, |n| format!(
                "%{n}$s"
            )),
            "%2$s for %1$s, 100%% {sure}"
        );
        assert_eq!(
            positional("100% {{sure}}", &[], |_| unreachable!()),
            "100% {sure}"
        );
    }
}
