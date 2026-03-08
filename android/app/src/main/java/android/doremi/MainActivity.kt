package android.doremi

import android.doremi.ui.theme.DoremiTheme
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import java.io.File

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            DoremiTheme() {
                val viewModel = remember {
                    NotesViewModel(loadNotes(this))
                }
                ViewNotes(viewModel)

                loadNotes(this).forEach {
                    saveNote(this, it)
                }
            }
        }
    }
}

private fun Note.toFileContent(): String = "$ctime\n$name\n${tags.joinToString(",")}\n$body"

private fun File.toNote(): Note {
    val lines = readLines()
    val ctime = lines.getOrNull(0)?.toLongOrNull() ?: 0L
    val name = lines.getOrNull(1) ?: ""
    val tags = lines.getOrNull(2)?.split(",")?.filter { it.isNotBlank() } ?: emptyList()
    val body = lines.drop(3).joinToString("\n")
    return Note(id = nameWithoutExtension, name = name, tags = tags, body = body, ctime = ctime)
}

private fun loadNotes(context: android.content.Context): List<Note> {
    val dir = File(context.filesDir, "notes")
    if (!dir.exists()) {
        dir.mkdirs()
        // seed the first time
//        TestData.notes.forEachIndexed { idx, testNote ->
//            val note = Note(
//                id = "note_$idx",
//                name = testNote.name,
//                tags = testNote.tags,
//                body = testNote.body,
//                ctime = System.currentTimeMillis()
//            )
//            val file = File(dir, "${note.id}.txt")
//            file.writeText(note.toFileContent())
//        }
    }
    return dir.listFiles()?.filter { it.extension == "txt" }?.map { it.toNote() } ?: emptyList()
}

private fun saveNote(context: android.content.Context, note: Note) {
    val dir = File(context.filesDir, "notes")
    if (!dir.exists()) dir.mkdirs()
    val file = File(dir, "${note.id}.txt")
    file.writeText(note.toFileContent())
}

data class Note(
    val id: String,
    val name: String,
    val tags: List<String>,
    val body: String,
    val ctime: Long = 0
)

class NotesViewModel(
    val notes: List<Note>
) : ViewModel() {
    // var notes = mutableStateListOf<Note>().also { it.addAll(notes) }

    // eg: one_word_match "two-word match" n:name t:"two-word tag" "b:three-word body content"
    var filter by mutableStateOf("")
}

@Composable
fun ViewNote(note: Note) {
    // TODO:
    //  - prettier (everything)
    //  - editing inline
    //  - clickable links in body
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 4.dp)
    ) {
        // TODO: move into viewmodel or something. there's only one note open at a time
        var isOpen by remember { mutableStateOf(false) }
        val surfaceColor by animateColorAsState(
            if (isOpen) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceVariant,
        )

        Column(
            modifier = Modifier
                .clickable { isOpen = !isOpen }
                .padding(12.dp)
        ) {
            Text(
                text = note.name,
                color = MaterialTheme.colorScheme.secondary,
                style = MaterialTheme.typography.titleSmall
            )
            if (note.tags.isNotEmpty()) {
                Text(
                    text = note.tags.joinToString(", "),
                    color = MaterialTheme.colorScheme.secondary,
                    style = MaterialTheme.typography.labelSmall
                )
            }
            Spacer(modifier = Modifier.height(4.dp))
            Surface(
                shape = MaterialTheme.shapes.medium,
                color = surfaceColor,
                modifier = Modifier
                    .animateContentSize()
                    .fillMaxWidth()
            ) {
                Text(
                    text = note.body,
                    modifier = Modifier.padding(horizontal = 4.dp),
                    maxLines = if (isOpen) Int.MAX_VALUE else 1,
                    style = MaterialTheme.typography.bodyMedium
                )
            }
        }
    }
}

@Composable
fun ViewNotes(vm: NotesViewModel) {
    Surface(modifier = Modifier.fillMaxSize()) {
        Column() {
            // TODO:
            //  - disappear after scroll down
            TextField(
                value = vm.filter,
                onValueChange = { vm.filter = it },
                label = { Text("Search") },
                modifier = Modifier.fillMaxWidth()
            )
            // TODO:
            //  - group by month/show month headers
            LazyColumn() {
                items(vm.notes) { note ->
                    ViewNote(note)
                }
            }
        }
    }
}

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
            "Lexi",
            listOf(),
            "I think Kotlin is my favorite programming language.\nIt's so much fun!"
        ),
        TestNote(
            "Lexi",
            listOf("Franco", "Programming", "Rust"),
            "Searching for alternatives to XML layouts..."
        ),
        TestNote(
            "Lexi",
            listOf(),
            "Hey, take a look at Jetpack Compose, it's great!\nIt's the Android's modern toolkit for building native UI.\nIt simplifies and accelerates UI development on Android.\nLess code, powerful tools, and intuitive Kotlin APIs :)"
        ),
        TestNote("Lexi", listOf(), "It's available from API 21+ : text box :)"),
        TestNote(
            "Lexi",
            listOf(),
            "Writing Kotlin for UI seems so natural, Compose where have you been all my life?"
        ),
        TestNote("Lexi", listOf(), "Android Studio next version's name is Arctic Fox"),
        TestNote(
            "Lexi",
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
