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
    placeholder: "Здесь появится дерево коллекций",
    resize: "Ширина боковой панели",
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
