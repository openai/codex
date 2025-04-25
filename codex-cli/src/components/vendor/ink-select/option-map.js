export default class OptionMap extends Map {
  first;
  constructor(options) {
    const items = [];
    let firstItem;
    let previous;
    let index = 0;
    for (const option of options) {
      const item = {
        ...option,
        previous,
        next: undefined,
        index,
      };
      if (previous) {
        previous.next = item;
      }
      firstItem ||= item;
      items.push([option.value, item]);
      index++;
      previous = item;
    }
    super(items);
    this.first = firstItem;
  }
}
