interface SearchBarProps {
  value: string;
  onInput: (value: string) => void;
  placeholder?: string;
}

export default function SearchBar(props: SearchBarProps) {
  return (
    <div class="field mb-5">
      <div class="control has-icons-left">
        <input
          class="input"
          type="text"
          placeholder={props.placeholder ?? "Search..."}
          value={props.value}
          onInput={(e) => props.onInput(e.currentTarget.value)}
        />
        <span class="icon is-left">🔍</span>
      </div>
    </div>
  );
}