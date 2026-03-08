package android.doremi

data class TestNote(val name: String, val tags: List<String>, val body: String)

object TestData {
    val notes = listOf(
        TestNote("Lexi", listOf("Pippo"), "Test...Test...Test..."),
        TestNote(
            "Lexi",
            listOf("Pippo", "pluto"),
            "List of Android versions:\nAndroid KitKat (API 19)\nAndroid Lollipop (API 21)\nAndroid Marshmallow (API 23)\nAndroid Nougat (API 24)\nAndroid Oreo (API 26)\nAndroid Pie (API 28)\nAndroid 10 (API 29)\nAndroid 11 (API 30)\nAndroid 12 (API 31)"
        ),
        TestNote(
            "Laxi",
            listOf(),
            "I think Kotlin is my favorite programming language.\nIt's so much fun!"
        ),
        TestNote(
            "Rust note",
            listOf("Franco", "Programming", "Rust"),
            "Searching for alternatives to XML layouts..."
        ),
        TestNote(
            "Programming note",
            listOf(),
            "Hey, take a look at Jetpack Compose, it's great!\nIt's the Android's modern toolkit for building native UI.\nIt simplifies and accelerates UI development on Android.\nLess code, powerful tools, and intuitive Kotlin APIs :)"
        ),
        TestNote("Lexi", listOf(), "It's available from API 21+ : text box :)"),
        TestNote(
            "Something",
            listOf(),
            "Writing Kotlin for UI seems so natural, Compose where have you been all my life?"
        ),
        TestNote("Lexi", listOf(), "Android Studio next version's name is Arctic Fox"),
        TestNote(
            "Android",
            listOf(),
            "Android Studio Arctic Fox tooling for Compose is top notch ^_^"
        ),
        TestNote(
            "Lexi",
            listOf(),
            "I didn't know you can now run the emulator directly from Android Studio"
        ),
        TestNote(
            "Lexi",
            listOf(),
            "Compose Previews are great to check quickly how a composable layout looks like"
        ),
        TestNote("Lexi", listOf(), "https://www.google.com"),
        TestNote("Lexi", listOf(), "Have you tried writing build.gradle with KTS?")
    )
}
