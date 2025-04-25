import { useInput } from "ink";
export const useSelect = ({ isDisabled = false, state }) => {
  useInput(
    (_input, key) => {
      if (key.downArrow) {
        state.focusNextOption();
      }
      if (key.upArrow) {
        state.focusPreviousOption();
      }
      if (key.return) {
        state.selectFocusedOption();
      }
    },
    { isActive: !isDisabled },
  );
};
