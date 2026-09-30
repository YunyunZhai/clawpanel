import { _ } from '../helper.js'

export default {
  title: _('日志查看', 'Logs', '日誌查看', 'ログ', '로그', 'Nhật ký', 'Registros', '', 'Журналы', 'Journaux', 'Protokolle'),
  desc: _('查看 OpenClaw 各服务日志', 'View OpenClaw service logs', '查看 OpenClaw 各服務日誌', 'OpenClaw サービスログを表示', 'OpenClaw 서비스 로그 보기', 'Xem nhật ký dịch vụ OpenClaw', 'Ver registros del servicio OpenClaw', 'Ver logs do serviço OpenClaw', 'Просмотр журналов OpenClaw', 'Voir les journaux OpenClaw', 'OpenClaw-Protokolle anzeigen'),
  noFiles: _('日志目录为空或尚未创建', 'No log files found', '日誌目錄為空或尚未建立', 'ログディレクトリが空です', '로그 디렉터리가 비어 있습니다', 'Không có tệp nhật ký', 'Sin archivos de registro', 'Nenhum arquivo de log', 'Файлы журналов не найдены', 'Aucun fichier journal', 'Keine Protokolldateien'),
  searchPlaceholder: _('搜索日志...', 'Search logs...', '搜尋日誌...', 'ログを検索...', '로그 검색...', 'Tìm kiếm...', 'Buscar...', 'Pesquisar...', 'Поиск...', 'Rechercher...', 'Suchen...'),
  refresh: _('刷新', 'Refresh', '重新整理', '更新', '새로고침', 'Làm mới', 'Actualizar', 'Atualizar', 'Обновить', 'Actualiser', 'Aktualisieren'),
  autoScroll: _('自动滚动', 'Auto scroll', '自動滚動', '自動スクロール', '자동 스크롤', 'Tự động cuộn', 'Auto-desplazar', 'Rolagem auto', 'Автопрокрутка', 'Défilement auto', 'Auto-Scrollen'),
  loading: _('加载日志中...', 'Loading logs...', '載入日誌中...', 'ログ読み込み中...', '로그 로딩 중...', 'Đang tải...', 'Cargando...', 'Carregando...', 'Загрузка...', 'Chargement...', 'Laden...'),
  empty: _('暂无日志', 'No logs', '暫無日誌', 'ログなし', '로그 없음', 'Không có nhật ký', 'Sin registros', 'Sem logs', 'Нет записей', 'Aucun journal', 'Keine Protokolle'),
  loadFailed: _('加载日志失败', 'Failed to load logs', '載入日誌失敗', 'ログの読み込みに失敗', '로그 로드 실패', 'Tải thất bại', 'Error al cargar', 'Falha ao carregar', 'Ошибка загрузки', 'Échec du chargement', 'Laden fehlgeschlagen'),
  noResults: _('未找到匹配结果', 'No matching results', '未找到匹配結果', '一致する結果なし', '일치하는 결과 없음', 'Không có kết quả', 'Sin resultados', 'Sem resultados', 'Ничего не найдено', 'Aucun résultat', 'Keine Ergebnisse'),
  searchFailed: _('搜索失败', 'Search failed', '搜尋失敗', '検索失敗', '검색 실패', 'Tìm kiếm thất bại', 'Búsqueda fallida', 'Pesquisa falhou', 'Ошибка поиска', 'Échec de la recherche', 'Suche fehlgeschlagen'),
  clearLog: _('清除日志', 'Clear Log', '清除日誌', 'ログを消去', '로그 삭제', 'Xóa nhật ký', 'Limpiar registro', 'Limpar log', 'Очистить журнал', 'Effacer le journal', 'Protokoll löschen'),
  clearConfirm: _('确认要清除日志文件 ${name}？此操作不可撤销。', 'Confirm to clear log file ${name}? This cannot be undone.', '確認要清除日誌檔案 ${name}？此操作無法撤銷。', 'ログファイル ${name} を消去してもよろしいですか？元に戻せません。', '로그 파일 ${name}을(를) 삭제하시겠습니까? 되돌릴 수 없습니다.', 'Xác nhận xóa tệp nhật ký ${name}? Không thể hoàn tác.', '¿Confirmar limpiar ${name}? No reversible.', 'Confirmar limpar ${name}? Irreversível.', 'Очистить файл журнала ${name}? Необратимо.', 'Confirmer la suppression de ${name} ?', '${name} löschen? Unwiderruflich.'),
  cleared: _('日志 ${name} 已清除', 'Log ${name} cleared', '日誌 ${name} 已清除', 'ログ ${name} を消去しました', '로그 ${name} 삭제됨', 'Đã xóa ${name}', '${name} limpiado', '${name} limpo', '${name} очищен', '${name} effacé', '${name} gelöscht'),
  clearFailed: _('清除日志失败', 'Failed to clear log', '清除日誌失敗', 'ログの消去に失敗', '로그 삭제 실패', 'Xóa thất bại', 'Error al limpiar', 'Falha ao limpar', 'Ошибка очистки', 'Échec de la suppression', 'Löschen fehlgeschlagen'),
}
