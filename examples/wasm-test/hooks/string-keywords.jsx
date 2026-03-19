/**
 * String keywords — reserved words in strings, JSX-like text (string-keywords-test style)
 */
export default function StringKeywords() {
  return (
    <div className="test-container">
      <h2 className="test-title">String Keywords Test</h2>
      <p className="text-xs text-gray-500 mb-2">
        Select a theme for the application
      </p>
      <button className="px-3 py-1 bg-blue-600 text-white rounded">
        Click Button
      </button>
      <div className="jsx-string-test">
        This string contains JSX-like text: {"<div>test</div>"}
      </div>
      <section className="mt-4 space-y-2">
        <div className="bg-gray-100 p-2 rounded">
          <p className="text-sm">Nested element with classes</p>
        </div>
      </section>
    </div>
  )
}
