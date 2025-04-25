const theme = {
  styles: {
    container: () => ({
      flexDirection: "column",
    }),
    option: ({ isFocused }) => ({
      gap: 1,
      paddingLeft: isFocused ? 0 : 2,
    }),
    selectedIndicator: () => ({
      color: "green",
    }),
    focusIndicator: () => ({
      color: "blue",
    }),
    label({ isFocused, isSelected }) {
      let color;
      if (isSelected) {
        color = "green";
      }
      if (isFocused) {
        color = "blue";
      }
      return { color };
    },
    highlightedText: () => ({
      bold: true,
    }),
  },
};
export const styles = theme.styles;
export default theme;
