// Every text of the interface. Keys are English; components contain no Russian strings.

export const strings = {
  units: {
    bytes: "Б",
    kilobytes: "КБ",
    megabytes: "МБ",
    milliseconds: "мс",
    seconds: "с",
  },
  sidebar: {
    label: "Коллекции",
    resize: "Ширина боковой панели",
    empty: "Перетащите сюда файл коллекции Postman или нажмите „Импорт“",
    filter: "Фильтр по имени",
    nothingFound: "Ничего не найдено",
  },
  import: {
    button: "Импорт",
    reportTitle: "Отчёт импорта",
    collection: "Коллекция",
    environment: "Окружение",
    globals: "Глобальные переменные",
    globalsCancelled: "Глобальные переменные не изменены: импорт отменён",
    reportKinds: {
      unsupported_auth: "Авторизация не поддерживается",
      setting_not_applied: "Настройки не применяются",
      has_scripts: "Скрипты",
      file_reference: "Ссылки на файлы",
    },
    conflictsTitle: "Глобальные переменные различаются",
    conflictsText: "Значения этих переменных в файле отличаются от текущих:",
    takeIncoming: "Взять из файла",
    keepCurrent: "Оставить текущие",
    cancel: "Отмена",
    close: "Закрыть",
  },
  environment: {
    label: "Окружение",
    none: "Без окружения",
  },
  tabs: {
    label: "Вкладки",
    placeholder: "Здесь появятся вкладки запросов",
  },
  editor: {
    label: "Редактор запроса",
    placeholder: "Здесь появится редактор запроса",
  },
  response: {
    label: "Ответ",
    placeholder: "Здесь появится ответ",
  },
  toasts: {
    label: "Уведомления",
  },
} as const;
