// Hindi (हिन्दी): each English sentence, as the code writes it, and its Hindi.
// Style and glossary: docs/07-design/I18N.md. scripts/i18n-check.ts checks that every
// string in the code is here, with the same {names}, and that nothing is left over.
export const hi: Readonly<Record<string, string>> = {
  // ==== Shared words, the app frame, Settings and banners ====

  // App frame and navigation
  Downloads: 'डाउनलोड',
  Send: 'भेजें',
  Networks: 'नेटवर्क',
  Settings: 'सेटिंग्स',
  Main: 'मुख्य',
  'New download': 'नया डाउनलोड',
  'Running in a browser: these transfers are simulated.':
    'ब्राउज़र में चल रहा है: ये ट्रांसफ़र नकली हैं।',
  'Demo data': 'डेमो डेटा',
  'Pick a download to see it fuse.': 'कोई डाउनलोड चुनें और देखें कि यह कैसे जुड़ता है।',
  'Starting Fuselane': 'Fuselane शुरू हो रहा है',
  'Skip to content': 'सीधे सामग्री पर जाएं',

  // Common buttons and words
  Download: 'डाउनलोड करें',
  Pause: 'रोकें',
  Resume: 'फिर शुरू करें',
  'Try again': 'फिर से कोशिश करें',
  Cancel: 'रद्द करें',
  Remove: 'हटाएं',
  Open: 'खोलें',
  'Show in folder': 'फ़ोल्डर में दिखाएं',
  Copy: 'कॉपी करें',
  Copied: 'कॉपी हो गया',
  Save: 'सेव करें',
  'Saved.': 'सेव हो गया।',
  Close: 'बंद करें',
  Back: 'वापस',
  Next: 'आगे',
  Change: 'बदलें',
  'Choose…': 'चुनें…',
  Done: 'हो गया',
  Later: 'बाद में',
  Dismiss: 'बंद करें',
  Name: 'नाम',
  Speed: 'स्पीड',
  Size: 'साइज़',
  Link: 'लिंक',
  Files: 'फ़ाइलें',
  Network: 'नेटवर्क',

  // Download states (status.ts)
  Waiting: 'इंतज़ार में',
  Downloading: 'डाउनलोड जारी',
  Paused: 'रुका हुआ',
  Stopped: 'रुका',
  Failed: 'विफल',
  Cancelled: 'रद्द',

  // File types (categories.ts)
  Video: 'वीडियो',
  Music: 'संगीत',
  Pictures: 'तस्वीरें',
  Documents: 'दस्तावेज़',
  Archives: 'आर्काइव',
  'Disk images': 'डिस्क इमेज',
  Apps: 'ऐप',
  Torrents: 'टोरेंट',
  Other: 'अन्य',

  // Kinds of network (lanes.ts)
  'USB tether': 'USB टेदर',
  Cellular: 'मोबाइल डेटा',
  Virtual: 'वर्चुअल',
  Loopback: 'लूपबैक',

  // Times and sizes (format.ts)
  'at {time}': '{time} पर',
  'tomorrow at {time}': 'कल {time} पर',
  '{day} at {time}': '{day} {time} पर',
  'Starts {when}': '{when} शुरू होगा',
  'Ready by {time}': '{time} तक तैयार',
  'Ready by tomorrow {time}': 'कल {time} तक तैयार',
  'Ready by {day} {time}': '{day} {time} तक तैयार',
  '{n} s left': '{n} सेकंड बाकी',
  '{n} min left': '{n} मिनट बाकी',
  '1 h {m} min left': '1 घंटा {m} मिनट बाकी',
  '{n} h {m} min left': '{n} घंटे {m} मिनट बाकी',

  // Settings (SettingsView.tsx)
  System: 'सिस्टम',
  Light: 'लाइट',
  Dark: 'डार्क',
  Theme: 'थीम',
  Language: 'भाषा',
  "System uses your computer's language.": 'सिस्टम चुनने पर आपके कंप्यूटर की भाषा दिखेगी।',
  'Speed unit': 'स्पीड की इकाई',
  'MB/s matches file sizes. Mbps matches how internet plans are sold (8 times bigger).':
    'MB/s फ़ाइलों के साइज़ से मेल खाता है। Mbps वैसे ही है जैसे इंटरनेट प्लान बेचे जाते हैं (8 गुना बड़ा)।',
  'Speed limit': 'स्पीड लिमिट',
  'Saved. Running downloads follow it now.': 'सेव हो गया। चल रहे डाउनलोड अब इसी पर चलेंगे।',
  'Limit removed.': 'लिमिट हटा दी गई।',
  'For all downloads and networks together.': 'सभी डाउनलोड और नेटवर्क के लिए मिलाकर।',
  'Speed limit for all networks': 'सभी नेटवर्क के लिए स्पीड लिमिट',
  'Slow mode': 'स्लो मोड',
  'One switch for calls and streaming: caps all downloads, then puts your normal limits back.':
    'कॉल और स्ट्रीमिंग के लिए एक स्विच: सभी डाउनलोड धीमे कर देता है, फिर आपकी सामान्य लिमिट वापस लगा देता है।',
  'Slow mode speed': 'स्लो मोड की स्पीड',
  'New download (or paste a link anywhere)': 'नया डाउनलोड (या कहीं भी लिंक पेस्ट करें)',
  'Move through downloads': 'डाउनलोड में ऊपर-नीचे जाएं',
  'Pause or resume the selected download': 'चुना हुआ डाउनलोड रोकें या फिर शुरू करें',
  'Back to the list, or close a dialog': 'सूची पर वापस जाएं, या डायलॉग बंद करें',
  'Keyboard shortcuts': 'कीबोर्ड शॉर्टकट',
  'Sharing is off. Finished torrents stop at once.':
    'शेयरिंग बंद है। पूरे हो चुके टोरेंट तुरंत रुक जाते हैं।',
  'Share torrents after downloading': 'डाउनलोड के बाद टोरेंट शेयर करें',
  'Uploads to other people for a while, then stops. Never over a phone tether or cellular while torrents are only sharing.':
    'कुछ देर दूसरों के लिए अपलोड करता है, फिर रुक जाता है। जब टोरेंट सिर्फ़ शेयर हो रहे हों, तब फ़ोन टेदर या मोबाइल डेटा पर कभी नहीं।',
  'Stop at ratio': 'इस रेशियो पर रोकें',
  '1 means upload as much as you downloaded.': '1 का मतलब है जितना डाउनलोड किया, उतना ही अपलोड।',
  'Stop after (minutes)': 'इतने मिनट बाद रोकें',
  'Whichever limit comes first ends sharing.': 'जो लिमिट पहले पूरी हो, शेयरिंग वहीं रुक जाती है।',
  'Check downloads against published checksums': 'डाउनलोड को साइट के चेकसम से जांचें',
  'Many sites put a SHA-256 next to the file (a .sha256 file or SHA256SUMS). Fuselane looks there before it starts and checks the finished file, so a damaged one is never saved.':
    'कई साइटें फ़ाइल के साथ SHA-256 देती हैं (.sha256 फ़ाइल या SHA256SUMS)। Fuselane शुरू करने से पहले उसे ढूंढता है और पूरी फ़ाइल जांचता है, ताकि ख़राब फ़ाइल कभी सेव न हो।',
  'Look up servers through each network': 'हर नेटवर्क से सर्वर खोजें',
  'Faster when your networks are from different providers: each one gets a server near it. Uses Cloudflare and Google DNS, which then see the names of the sites you download from (never the files).':
    'जब आपके नेटवर्क अलग-अलग कंपनियों के हों, तब ज़्यादा तेज़: हर नेटवर्क को अपने पास का सर्वर मिलता है। इसमें Cloudflare और Google DNS इस्तेमाल होते हैं, जिन्हें उन साइटों के नाम दिखते हैं जहां से आप डाउनलोड करते हैं (फ़ाइलें कभी नहीं)।',
  Updates: 'अपडेट',
  'Fuselane checks a signed update feed. Nothing about you is sent.':
    'Fuselane एक साइन किया हुआ अपडेट फ़ीड देखता है। आपके बारे में कुछ नहीं भेजा जाता।',
  "You're up to date.": 'आपके पास सबसे नया वर्ज़न है।',
  'Updates come through your software centre (Flatpak).':
    'अपडेट आपके सॉफ़्टवेयर सेंटर (Flatpak) से आते हैं।',
  'Browser extension': 'ब्राउज़र एक्सटेंशन',
  "The browser extension can't talk to the Flatpak version yet; use the .deb or AppImage for it.":
    'ब्राउज़र एक्सटेंशन अभी Flatpak वर्ज़न से बात नहीं कर सकता; इसके लिए .deb या AppImage इस्तेमाल करें।',
  "The Flatpak version can't put the computer to sleep or shut it down.":
    'Flatpak वर्ज़न कंप्यूटर को स्लीप या बंद नहीं कर सकता।',
  'Version {version} is ready to install.': 'वर्ज़न {version} इंस्टॉल के लिए तैयार है।',
  'Checking…': 'जांच हो रही है…',
  'Check again': 'फिर से जांचें',
  'Check for updates': 'अपडेट देखें',
  'Download list': 'डाउनलोड सूची',
  'Save every link to a text file, or add links from one. Imported downloads wait until you start them.':
    'सभी लिंक एक टेक्स्ट फ़ाइल में सेव करें, या किसी फ़ाइल से लिंक जोड़ें। इंपोर्ट किए गए डाउनलोड तब तक रुके रहते हैं, जब तक आप उन्हें शुरू न करें।',
  'Saved 1 link.': '1 लिंक सेव हुआ।',
  'Saved {n} links.': '{n} लिंक सेव हुए।',
  'Export…': 'एक्सपोर्ट करें…',
  'Import…': 'इंपोर्ट करें…',
  'Added 1 download; skipped {skipped} already in the list or not valid.':
    '1 डाउनलोड जोड़ा गया; {skipped} छोड़ दिए, जो पहले से सूची में थे या सही नहीं थे।',
  'Added {n} downloads; skipped {skipped} already in the list or not valid.':
    '{n} डाउनलोड जोड़े गए; {skipped} छोड़ दिए, जो पहले से सूची में थे या सही नहीं थे।',
  'Added 1 download.': '1 डाउनलोड जोड़ा गया।',
  'Added {n} downloads.': '{n} डाउनलोड जोड़े गए।',
  'Copied. Paste it into your bug report.': 'कॉपी हो गया। इसे अपनी बग रिपोर्ट में पेस्ट करें।',
  'Select the text below and copy it.': 'नीचे का टेक्स्ट चुनकर कॉपी करें।',
  "Couldn't build the report. Try again.": 'रिपोर्ट नहीं बन सकी। फिर से कोशिश करें।',
  Diagnostics: 'डायग्नोस्टिक्स',
  'For bug reports. It never includes IP addresses, links or file names, and Fuselane sends nothing by itself. Report a problem opens a GitHub issue with it filled in.':
    'बग रिपोर्ट के लिए। इसमें IP पते, लिंक या फ़ाइलों के नाम कभी नहीं होते, और Fuselane अपने आप कुछ नहीं भेजता। “समस्या बताएं” इसे भरकर GitHub पर एक रिपोर्ट खोलता है।',
  'Collecting…': 'जानकारी जुटा रहे हैं…',
  'Copy diagnostics': 'डायग्नोस्टिक्स कॉपी करें',
  'Opens a bug report on GitHub with the diagnostics filled in; you read it before sending':
    'GitHub पर डायग्नोस्टिक्स भरी हुई बग रिपोर्ट खोलता है; भेजने से पहले आप उसे पढ़ते हैं',
  'Report a problem': 'समस्या बताएं',
  'Diagnostics report': 'डायग्नोस्टिक्स रिपोर्ट',
  'Look and feel': 'रूप-रंग',
  'This computer': 'यह कंप्यूटर',
  'Torrents and lookups': 'टोरेंट और सर्वर खोज',
  'Other apps': 'दूसरे ऐप',
  About: 'जानकारी',
  Appearance: 'थीम',
  'Follows your system unless you pick one.': 'जब तक आप कोई न चुनें, सिस्टम के हिसाब से चलता है।',
  'Downloads folder': 'डाउनलोड फ़ोल्डर',
  Welcome: 'स्वागत',
  'The short tour from the first launch: networks, a check, tips.':
    'पहली बार खोलने पर दिखा छोटा परिचय: नेटवर्क, एक जांच, सुझाव।',
  'Show it again': 'फिर से दिखाएं',
  Version: 'वर्ज़न',
  '{version}, demo data': '{version}, डेमो डेटा',

  // The window shown when the download list can't be opened at launch (StartupProblem)
  'Your download list is from a newer Fuselane': 'आपकी डाउनलोड सूची Fuselane के नए वर्ज़न की है',
  'This copy is version {version}. Update to keep going. Your downloads are kept.':
    'यह कॉपी वर्ज़न {version} है। आगे बढ़ने के लिए अपडेट करें। आपके डाउनलोड सुरक्षित रहेंगे।',
  'Update now': 'अभी अपडेट करें',
  'Looking for the update…': 'अपडेट ढूंढा जा रहा है…',
  'No update was found for this copy.': 'इस कॉपी के लिए कोई अपडेट नहीं मिला।',
  'Get the newest Fuselane from fuselane.app and install it over this one. Your downloads are kept.':
    'fuselane.app से नया Fuselane लें और इसी के ऊपर इंस्टॉल करें। आपके डाउनलोड सुरक्षित रहेंगे।',
  'You can also get the newest Fuselane from fuselane.app. Your downloads are kept.':
    'आप fuselane.app से भी नया Fuselane ले सकते हैं। आपके डाउनलोड सुरक्षित रहेंगे।',
  'Get Fuselane from fuselane.app': 'fuselane.app से Fuselane लें',
  "Fuselane can't start": 'Fuselane शुरू नहीं हो पा रहा',
  'Error details': 'गड़बड़ी की जानकारी',
  'Open the folder': 'फ़ोल्डर खोलें',
  'Copy details': 'जानकारी कॉपी करें',

  // Banners (UpdateBanner, CrashBanner, WhenDoneBanner, SlowMode) and the store's toasts
  'Installing {version}.': '{version} इंस्टॉल हो रहा है।',
  'Fuselane restarts in a moment and your downloads carry on.':
    'Fuselane थोड़ी देर में रीस्टार्ट होगा और आपके डाउनलोड चलते रहेंगे।',
  'Downloading Fuselane {version}': 'Fuselane {version} डाउनलोड हो रहा है',
  'over {n} networks': '{n} नेटवर्क से',
  'Update download': 'अपडेट का डाउनलोड',
  '{done} of {total}': '{total} में से {done}',
  'Fuselane {version} is ready to download, {size}.':
    'Fuselane {version} डाउनलोड के लिए तैयार है, {size}।',
  'Fuselane {version} is ready to download.': 'Fuselane {version} डाउनलोड के लिए तैयार है।',
  'Downloads pause and continue after the restart.':
    'डाउनलोड रुकेंगे और रीस्टार्ट के बाद फिर चलेंगे।',
  'Update and restart': 'अपडेट करके रीस्टार्ट करें',
  'Fuselane was updated to {version}.': 'Fuselane {version} पर अपडेट हो गया।',
  'Your downloads and settings are kept.': 'आपके डाउनलोड और सेटिंग्स सुरक्षित हैं।',
  "What's new": 'नया क्या है',
  "Fuselane closed unexpectedly last time. Sending a report helps fix it; you see everything it says before it's sent.":
    'पिछली बार Fuselane अचानक बंद हो गया था। रिपोर्ट भेजने से इसे ठीक करने में मदद मिलती है; भेजने से पहले आप उसमें लिखा सब कुछ देख सकते हैं।',
  'Report it': 'रिपोर्ट भेजें',
  'All downloads finished. Your computer goes to sleep in {n} second.':
    'सभी डाउनलोड पूरे हुए। आपका कंप्यूटर {n} सेकंड में स्लीप हो जाएगा।',
  'All downloads finished. Your computer goes to sleep in {n} seconds.':
    'सभी डाउनलोड पूरे हुए। आपका कंप्यूटर {n} सेकंड में स्लीप हो जाएगा।',
  'All downloads finished. Your computer shuts down in {n} second.':
    'सभी डाउनलोड पूरे हुए। आपका कंप्यूटर {n} सेकंड में बंद हो जाएगा।',
  'All downloads finished. Your computer shuts down in {n} seconds.':
    'सभी डाउनलोड पूरे हुए। आपका कंप्यूटर {n} सेकंड में बंद हो जाएगा।',
  'All downloads finished. Fuselane quits in {n} second.':
    'सभी डाउनलोड पूरे हुए। Fuselane {n} सेकंड में बंद हो जाएगा।',
  'All downloads finished. Fuselane quits in {n} seconds.':
    'सभी डाउनलोड पूरे हुए। Fuselane {n} सेकंड में बंद हो जाएगा।',
  '{rate} max': 'ज़्यादा से ज़्यादा {rate}',
  'That file is {size} MiB; a .torrent file is at most 8 MiB.':
    'यह फ़ाइल {size} MiB की है; .torrent फ़ाइल ज़्यादा से ज़्यादा 8 MiB की होती है।',
  'Something went wrong.': 'कुछ गड़बड़ हो गई।',
  'Try again. If it keeps happening, restart Fuselane.':
    'फिर से कोशिश करें। अगर ऐसा बार-बार हो, तो Fuselane रीस्टार्ट करें।',
  'Drop the .torrent file, not the download itself.':
    '.torrent फ़ाइल डालें, डाउनलोड की गई फ़ाइल नहीं।',

  // ==== The download list and details ====

  // TransferList
  Saved: 'सेव हुआ',
  'Has every network': 'सभी नेटवर्क इसी डाउनलोड पर',
  '{ready}, at risk': '{ready}, देर हो सकती है',
  'Pause {name}': '{name} रोकें',
  'Resume {name}': '{name} फिर शुरू करें',
  '{size}, shared {shared}': '{size}, {shared} शेयर किया',
  'Group name': 'ग्रुप का नाम',
  Rename: 'नाम बदलें',
  Ungroup: 'अलग करें',
  '{done} of {total} done': '{total} में से {done} पूरे',
  'Pause all in {name}': '{name} में सब रोकें',
  'Resume all in {name}': '{name} में सब फिर शुरू करें',
  'Pause all': 'सब रोकें',
  'Resume all': 'सब फिर शुरू करें',
  All: 'सभी',
  Active: 'चालू',
  Finished: 'पूरे',
  'Loading downloads': 'डाउनलोड लोड हो रहे हैं',
  'Nothing downloading yet': 'अभी कुछ डाउनलोड नहीं हो रहा',
  'Paste a link or a magnet, or drop a .torrent file. Fuselane spreads it across every network you have, then fuses the parts into one file.':
    'कोई लिंक या मैग्नेट पेस्ट करें, या .torrent फ़ाइल यहां छोड़ें। Fuselane इसे आपके हर नेटवर्क पर बांटता है, फिर सारे हिस्से जोड़कर एक फ़ाइल बनाता है।',
  'Follow a feed': 'फ़ीड फ़ॉलो करें',
  'Search downloads…': 'डाउनलोड खोजें…',
  'Search downloads': 'डाउनलोड खोजें',
  'Follow podcasts, releases and other feeds: new files download by themselves':
    'पॉडकास्ट, रिलीज़ और दूसरी फ़ीड फ़ॉलो करें: नई फ़ाइलें अपने आप डाउनलोड होंगी',
  Feeds: 'फ़ीड',
  Show: 'दिखाएं',
  Type: 'प्रकार',
  'All types': 'सभी प्रकार',
  'Nothing matches "{query}".': '"{query}" से कुछ नहीं मिला।',
  'Nothing here.': 'यहां कुछ नहीं है।',
  'Show everything': 'सब दिखाएं',
  Groups: 'ग्रुप',
  Recent: 'हाल के',

  // TransferDetail
  'Show in Finder': 'Finder में दिखाएं',
  'Show in Explorer': 'Explorer में दिखाएं',
  '{times}x faster than {network}': '{network} से {times}x तेज़',
  '{n} network': '{n} नेटवर्क',
  '{n} networks': '{n} नेटवर्क',
  '{p} percent, {rate}': '{p} प्रतिशत, {rate}',
  'Fuselane tries again by itself in under a minute, or as soon as a network comes back.':
    'Fuselane एक मिनट के अंदर, या कोई नेटवर्क लौटते ही, अपने आप फिर से कोशिश करेगा।',
  'Fuselane tries again by itself in about {n} min, or as soon as a network comes back.':
    'Fuselane लगभग {n} मिनट में, या कोई नेटवर्क लौटते ही, अपने आप फिर से कोशिश करेगा।',
  'New link to the same file': 'उसी फ़ाइल का नया लिंक',
  Continue: 'जारी रखें',
  "Saved progress is kept if it's the same file. A different file is never mixed in.":
    'अगर फ़ाइल वही है तो अब तक का डाउनलोड बना रहता है। कोई दूसरी फ़ाइल कभी नहीं मिलाई जाती।',
  'Start over': 'शुरू से करें',
  'Open Networks': 'नेटवर्क खोलें',
  'Free up space on that disk, or choose another folder, then try again.':
    'उस डिस्क पर जगह खाली करें या कोई दूसरा फ़ोल्डर चुनें, फिर से कोशिश करें।',
  'Speed limit for this download': 'इस डाउनलोड की स्पीड लिमिट',
  'Remove limit': 'लिमिट हटाएं',
  'Set limit': 'लिमिट लगाएं',
  'Empty for no limit. The overall and network limits still apply.':
    'लिमिट न चाहिए तो खाली छोड़ें। कुल और नेटवर्क वाली लिमिट फिर भी लागू रहती हैं।',
  'On track': 'समय पर है',
  'At risk: it goes first, and runs outside the schedule if it has to':
    'देर हो सकती है: यह सबसे पहले चलेगा, और ज़रूरत हो तो शेड्यूल के बाहर भी',
  'The time has passed; it carries on': 'समय निकल गया; डाउनलोड चलता रहेगा',
  'Ready by': 'कब तक तैयार',
  Set: 'लगाएं',
  Clear: 'हटाएं',
  '{ready}. {state}.': '{ready}। {state}।',
  'Downloads with a time go first, earliest first.':
    'जिन डाउनलोड का समय तय है वे पहले चलते हैं, सबसे जल्दी वाला सबसे पहले।',
  "Send it, with what's downloaded so far, to another computer with Fuselane":
    'अब तक डाउनलोड हुए हिस्से के साथ इसे Fuselane वाले दूसरे कंप्यूटर पर भेजें',
  'Continue elsewhere': 'कहीं और जारी रखें',
  'Remove this download?': 'यह डाउनलोड हटाएं?',
  'Stop and remove this download?': 'यह डाउनलोड रोककर हटाएं?',
  '{name} leaves your list. Keep the file, or move it to the {bin}?':
    '{name} आपकी सूची से हट जाएगा। फ़ाइल रखें, या उसे {bin} में भेजें?',
  "{name} stops and leaves your list. The unfinished file can't be used, so it's deleted.":
    '{name} रुककर आपकी सूची से हट जाएगा। अधूरी फ़ाइल किसी काम की नहीं, इसलिए वह मिटा दी जाएगी।',
  '{done} of {total} downloaded': '{total} में से {done} डाउनलोड हुआ',
  '{done} downloaded': '{done} डाउनलोड हुआ',
  'Keep file': 'फ़ाइल रखें',
  'Move file to {bin}': 'फ़ाइल {bin} में भेजें',
  'Delete unfinished file': 'अधूरी फ़ाइल मिटाएं',
  '{n} s': '{n} सेकंड',
  '{n} min': '{n} मिनट',
  '1 h {m} min': '1 घंटा {m} मिनट',
  '{n} h {m} min': '{n} घंटे {m} मिनट',
  '1 h': '1 घंटा',
  '{n} h': '{n} घंटे',
  'What each network saved': 'हर नेटवर्क ने कितना समय बचाया',
  'Finished in {time}. Without {network} it would have taken about {longer}.':
    '{time} में पूरा हुआ। {network} के बिना लगभग {longer} लगते।',
  'Finished in {time}.': '{time} में पूरा हुआ।',
  '{time} saved': '{time} बचे',
  'against {source}': '{source} से',
  'your SHA-256': 'आपके SHA-256',
  'The finished file matches the SHA-256 from {source}.':
    'पूरी हुई फ़ाइल {source} वाले SHA-256 से मेल खाती है।',
  'The finished file matches the SHA-256 you gave.':
    'पूरी हुई फ़ाइल आपके दिए SHA-256 से मेल खाती है।',
  Verified: 'सत्यापित',
  'When it finishes, the file is checked against the SHA-256 from {source}.':
    'पूरी होने पर फ़ाइल को {source} वाले SHA-256 से जांचा जाएगा।',
  'When it finishes, the file is checked against the SHA-256 you gave.':
    'पूरी होने पर फ़ाइल को आपके दिए SHA-256 से जांचा जाएगा।',
  'Checked when done': 'पूरा होने पर जांच',
  'Back to downloads': 'डाउनलोड पर वापस',
  'Starts as soon as a download finishes': 'कोई डाउनलोड पूरा होते ही शुरू होगा',
  'Start next': 'इसे अगला चलाएं',
  'Every network goes to this download; the others wait and carry on after it':
    'सभी नेटवर्क इस डाउनलोड को मिलेंगे; बाकी इंतज़ार करेंगे और इसके बाद चलेंगे',
  'Do this now': 'इसे अभी करें',
  'Let the other downloads run again': 'बाकी डाउनलोड फिर से चलने दें',
  'Every network': 'सभी नेटवर्क',
  'Start now': 'अभी शुरू करें',
  'of {total}': '/ {total}',
  'Time left': 'बचा समय',
  Status: 'स्थिति',
  'Working it out': 'हिसाब लग रहा है',
  Streams: 'कनेक्शन',
  Recovered: 'रिकवरी',
  '{n} retry': '{n} बार दोबारा कोशिश',
  '{n} retries': '{n} बार दोबारा कोशिश',
  '{n} race': '{n} रेस',
  '{n} races': '{n} रेस',
  'Also from a mirror: {mirrors}. Each was checked for the same file before helping.':
    'एक मिरर से भी: {mirrors}। मदद से पहले जांचा गया कि उस पर वही फ़ाइल है।',
  'Also from {n} mirrors: {mirrors}. Each was checked for the same file before helping.':
    '{n} मिरर से भी: {mirrors}। मदद से पहले हर एक पर जांचा गया कि फ़ाइल वही है।',
  'Networks in this download': 'इस डाउनलोड के नेटवर्क',
  Share: 'हिस्सा',
  Carried: 'लाया गया',
  Offline: 'ऑफ़लाइन',

  // TorrentDetail
  Checking: 'जांच हो रही है',
  Sharing: 'शेयर हो रहा है',
  'Remove this torrent?': 'यह टोरेंट हटाएं?',
  "{name} leaves your list. What should happen to what's already saved?":
    '{name} आपकी सूची से हट जाएगा। जो पहले से सेव है, उसका क्या करें?',
  "{name} stops and leaves your list. What should happen to what's already saved?":
    '{name} रुककर आपकी सूची से हट जाएगा। जो पहले से सेव है, उसका क्या करें?',
  '{size}, {selected} of {count} files': '{size}, {count} में से {selected} फ़ाइलें',
  '{done} of {total} saved': '{total} में से {done} सेव हुआ',
  'Keep files': 'फ़ाइलें रखें',
  'Move files to {bin}': 'फ़ाइलें {bin} में भेजें',
  'Loading peers': 'पीयर लोड हो रहे हैं',
  'Looking for peers…': 'पीयर खोजे जा रहे हैं…',
  'No peers connected right now.': 'अभी कोई पीयर जुड़ा नहीं है।',
  'Connected peers': 'जुड़े हुए पीयर',
  'Unknown app': 'अनजान ऐप',
  'Came to you': 'खुद आपसे जुड़ा',
  'From this peer': 'इस पीयर से',
  'To this peer': 'इस पीयर को',
  Down: 'डाउनलोड',
  Up: 'अपलोड',
  High: 'ज़्यादा',
  Normal: 'सामान्य',
  Low: 'कम',
  Skip: 'छोड़ें',
  'Fetched before the others': 'बाकी से पहले आती है',
  'Waits until the rest is done': 'बाकी पूरा होने तक रुकती है',
  'Not downloaded': 'डाउनलोड नहीं होगी',
  'Loading files': 'फ़ाइलें लोड हो रही हैं',
  '{size} to download': '{size} डाउनलोड होना है',
  'Files in this torrent': 'इस टोरेंट की फ़ाइलें',
  Here: 'यहां',
  Priority: 'प्राथमिकता',
  Play: 'चलाएं',
  'Priority of {name}': '{name} की प्राथमिकता',
  'Play {name}': '{name} चलाएं',
  "Playing from the start while the rest arrives. If it doesn't open in your player, paste this link into one (VLC, IINA, mpv):":
    'शुरू से चल रहा है, बाकी साथ-साथ आ रहा है। अगर यह आपके प्लेयर में न खुले, तो यह लिंक किसी प्लेयर (VLC, IINA, mpv) में पेस्ट करें:',
  'Stream link': 'स्ट्रीम लिंक',
  'High files come first and Low ones wait for the rest. Play fetches the start of a video or song first, so it can begin while the rest arrives.':
    'ज़्यादा प्राथमिकता वाली फ़ाइलें पहले आती हैं और कम वाली बाकी का इंतज़ार करती हैं। चलाएं दबाने पर वीडियो या गाने की शुरुआत पहले आती है, ताकि बाकी आते-आते वह चलने लगे।',
  Peers: 'पीयर',
  'Stop sharing': 'शेयर करना बंद करें',
  Shared: 'शेयर किया',
  'Pieces: {pct}% here': 'टुकड़े: {pct}% यहां',
  'Torrent details': 'टोरेंट की जानकारी',
  'Networks in this torrent': 'इस टोरेंट के नेटवर्क',
  'All networks': 'सभी नेटवर्क',
  'Speed is what each network receives right now. Share counts only pieces that passed their checksum.':
    'स्पीड बताती है कि हर नेटवर्क अभी कितना ला रहा है। हिस्से में सिर्फ़ वे टुकड़े गिने जाते हैं जिनका चेकसम सही निकला।',
  'Each network is credited only with pieces that passed their checksum.':
    'हर नेटवर्क के नाम सिर्फ़ वे टुकड़े गिने जाते हैं जिनका चेकसम सही निकला।',

  // FuseCore, SpeedSplit, Stream
  'Speed by network': 'हर नेटवर्क की स्पीड',
  'adds up to the speed above': 'मिलकर ऊपर वाली स्पीड बनती है',
  '{n} s ago': '{n} सेकंड पहले',
  now: 'अभी',

  // RemoveDialog
  'Recycle Bin': 'रीसायकल बिन',
  Trash: 'ट्रैश',

  // HandoffDialog
  'Continue on another computer': 'दूसरे कंप्यूटर पर जारी रखें',
  'Sent to {device}. It appears there paused; press Resume on that computer to carry on. You can remove it here.':
    '{device} पर भेज दिया। वहां यह रुका हुआ दिखेगा; आगे बढ़ाने के लिए उस कंप्यूटर पर "फिर शुरू करें" दबाएं। यहां से आप इसे हटा सकते हैं।',
  "What's downloaded so far goes along, so the other computer carries on where this one stopped. Both need Fuselane, on the same network.":
    'अब तक जितना डाउनलोड हुआ है वह भी साथ जाता है, ताकि दूसरा कंप्यूटर वहीं से आगे बढ़े जहां यह रुका था। दोनों पर Fuselane होना चाहिए, एक ही नेटवर्क पर।',
  'Sending…': 'भेजा जा रहा है…',
  'Looking for other computers with Fuselane… Open Fuselane on the other computer.':
    'Fuselane वाले दूसरे कंप्यूटर खोजे जा रहे हैं… दूसरे कंप्यूटर पर Fuselane खोलें।',

  // ==== Adding downloads, and feeds ====

  // New download (NewDownload.tsx)
  'Torrent {hash}': 'टोरेंट {hash}',
  'Get the video': 'वीडियो लें',
  '{site}, {duration}. Downloads over every network at once.':
    '{site}, {duration}। हर नेटवर्क से एक साथ डाउनलोड होगा।',
  '{site}. Downloads over every network at once.': '{site}। हर नेटवर्क से एक साथ डाउनलोड होगा।',
  Quality: 'क्वालिटी',
  'Higher qualities need the free ffmpeg tool to join video and sound. Install it and look the page up again.':
    'ऊंची क्वालिटी के लिए वीडियो और आवाज़ जोड़ने वाला मुफ़्त ffmpeg टूल चाहिए। इसे इंस्टॉल करके पेज फिर से देखें।',
  'Adding…': 'जोड़ रहे हैं…',
  'Files on the page': 'पेज पर फ़ाइलें',
  '{n} file. Pick what to download; more than one stay together as a group.':
    '{n} फ़ाइल। चुनें कि क्या डाउनलोड करना है; एक से ज़्यादा फ़ाइलें एक ग्रुप में साथ रहेंगी।',
  '{n} files. Pick what to download; more than one stay together as a group.':
    '{n} फ़ाइलें। चुनें कि क्या डाउनलोड करना है; एक से ज़्यादा फ़ाइलें एक ग्रुप में साथ रहेंगी।',
  'Download {n}': '{n} डाउनलोड करें',
  'Choose files': 'फ़ाइलें चुनें',
  'Saves to {folder}': '{folder} में सेव होगा',
  'Starting…': 'शुरू हो रहा है…',
  Links: 'लिंक',
  'Magnet link, {hash}': 'मैग्नेट लिंक, {hash}',
  'Change the link': 'लिंक बदलें',
  'https://example.com/file.iso or magnet:?…': 'https://example.com/file.iso या magnet:?…',
  'Looking up the file…': 'फ़ाइल देख रहे हैं…',
  '{name}, {size}.': '{name}, {size}।',
  '{name}, size unknown.': '{name}, साइज़ पता नहीं।',
  'Splits across all your networks.': 'आपके सभी नेटवर्क में बंटकर आएगा।',
  "This server won't split the file, so one network will carry it.":
    'यह सर्वर फ़ाइल को बांटने नहीं देता, इसलिए एक ही नेटवर्क इसे लाएगा।',
  '{n} link with a pattern. Each [01-20] becomes one download per number.':
    'पैटर्न वाला {n} लिंक। हर [01-20] से हर नंबर का एक डाउनलोड बनेगा।',
  '{n} links with a pattern. Each [01-20] becomes one download per number.':
    'पैटर्न वाले {n} लिंक। हर [01-20] से हर नंबर का एक डाउनलोड बनेगा।',
  '{n} links. Each becomes its own download; ones already in your list are skipped.':
    '{n} लिंक। हर लिंक का अपना डाउनलोड बनेगा; जो पहले से आपकी सूची में हैं, वे छोड़ दिए जाएंगे।',
  'A Metalink: Fuselane downloads the files it lists, each from all its mirrors, and checks them.':
    'यह Metalink है: Fuselane इसमें दी गई हर फ़ाइल को उसके सभी मिरर से डाउनलोड करता है और जांचता है।',
  "Finding the torrent's files. This can take a minute when few people share it.":
    'टोरेंट की फ़ाइलें खोज रहे हैं। कम लोग शेयर कर रहे हों तो इसमें एक मिनट लग सकता है।',
  'A magnet link. Next you pick which of its files to download.':
    'यह मैग्नेट लिंक है। आगे आप चुनेंगे कि इसकी कौन-सी फ़ाइलें डाउनलोड करनी हैं।',
  'Fuselane uses every network that can reach the server.':
    'Fuselane हर उस नेटवर्क का इस्तेमाल करता है जो सर्वर तक पहुंच सकता है।',
  'Reading the page…': 'पेज पढ़ रहे हैं…',
  'Find files on this page': 'इस पेज पर फ़ाइलें खोजें',
  'Looking for the video…': 'वीडियो खोज रहे हैं…',
  'Get the video from this page': 'इस पेज से वीडियो लें',
  'Keep them together as a group': 'इन्हें एक ग्रुप में साथ रखें',
  'One row in your list with one progress, and one notice when all are done.':
    'आपकी सूची में एक ही लाइन, एक ही प्रोग्रेस, और सब पूरे होने पर एक ही सूचना।',
  'Name (optional)': 'नाम (ज़रूरी नहीं)',
  "You already downloaded this ({size}, {date}). It's still in this folder.":
    'आप इसे पहले ही डाउनलोड कर चुके हैं ({size}, {date})। यह अब भी इस फ़ोल्डर में है।',
  'A file named {name} is already in this folder.':
    '{name} नाम की फ़ाइल इस फ़ोल्डर में पहले से है।',
  'Keep both': 'दोनों रखें',
  Replace: 'बदल दें',
  "Don't download": 'डाउनलोड न करें',
  'Save to': 'कहां सेव करें',
  'Leave empty to use {folder}.': '{folder} इस्तेमाल करने के लिए खाली छोड़ें।',
  'Leave empty to use your Downloads folder.':
    'अपना डाउनलोड फ़ोल्डर इस्तेमाल करने के लिए खाली छोड़ें।',
  'More options': 'और विकल्प',
  'Save as': 'इस नाम से सेव करें',
  "The server's name…": 'सर्वर का दिया नाम…',
  'Leave empty to keep the name the server gives it.': 'सर्वर का दिया नाम रखने के लिए खाली छोड़ें।',
  'SHA-256 to check': 'जांचने के लिए SHA-256',
  '64 letters and digits from the download page…': 'डाउनलोड पेज से 64 अक्षर और अंक…',
  "Fuselane checks the finished file and won't save it under its name if it differs.":
    'Fuselane पूरी हुई फ़ाइल जांचता है, और अगर वह अलग निकली तो उसे उसके नाम से सेव नहीं करता।',
  Mirrors: 'मिरर',
  'Mirror {n}': 'मिरर {n}',
  'Remove mirror {n}': 'मिरर {n} हटाएं',
  'Add a mirror': 'मिरर जोड़ें',
  'The same file on other servers. Each is checked first, then every network fetches from all of them.':
    'यही फ़ाइल दूसरे सर्वर पर। पहले हर एक की जांच होती है, फिर हर नेटवर्क सभी से डाउनलोड करता है।',
  Start: 'शुरू',
  Now: 'अभी',
  'At a time': 'तय समय पर',
  'Start time': 'शुरू होने का समय',
  'It waits in your list and starts by itself, even with the window closed.':
    'यह आपकी सूची में इंतज़ार करेगा और विंडो बंद होने पर भी अपने आप शुरू होगा।',
  'Starts as soon as there is room in the queue.': 'कतार में जगह मिलते ही शुरू होगा।',
  "You already downloaded this: {name}, {size}, on {date}. It's still in {folder}.":
    'आप इसे पहले ही डाउनलोड कर चुके हैं: {name}, {size}, {date} को। यह अब भी {folder} में है।',
  'Download again': 'फिर से डाउनलोड करें',
  "One link was not added. The rest started. They're left in the box above:":
    'एक लिंक नहीं जुड़ा। बाकी शुरू हो गए। यह ऊपर बॉक्स में छोड़ा गया है:',
  "{n} links were not added. The rest started. They're left in the box above:":
    '{n} लिंक नहीं जुड़े। बाकी शुरू हो गए। ये ऊपर बॉक्स में छोड़े गए हैं:',
  'and {n} more.': 'और {n}।',
  'Open .torrent…': '.torrent खोलें…',
  'Add it to the list without starting it': 'शुरू किए बिना सूची में जोड़ें',
  'Download later': 'बाद में डाउनलोड करें',
  'Finding files…': 'फ़ाइलें खोज रहे हैं…',
  'Download all': 'सभी डाउनलोड करें',
  'Download at {time}': '{time} पर डाउनलोड करें',

  // Torrent files (FilePicker.tsx)
  'Files to download': 'डाउनलोड करने वाली फ़ाइलें',
  'All files': 'सभी फ़ाइलें',
  '{chosen} of {total}, {size}': '{total} में से {chosen}, {size}',

  // Files on a page (PagePicker.tsx)
  'Pick by type': 'प्रकार से चुनें',
  'All {n}': 'सभी {n}',

  // Speed limit field (LimitField.tsx)
  'No limit': 'कोई लिमिट नहीं',
  '{label} unit': '{label} की इकाई',
  'Enter a number, like 5 or 2.5.': 'कोई संख्या लिखें, जैसे 5 या 2.5।',

  // Feeds (FeedsDialog.tsx)
  'Every 15 minutes': 'हर 15 मिनट',
  'Every hour': 'हर घंटे',
  'Every 6 hours': 'हर 6 घंटे',
  'Once a day': 'दिन में एक बार',
  'Skipped by your words': 'आपके शब्दों से छोड़ा गया',
  'No file in it': 'इसमें कोई फ़ाइल नहीं',
  Torrent: 'टोरेंट',
  "Couldn't add": 'नहीं जुड़ सका',
  'Torrent started': 'टोरेंट शुरू हुआ',
  'Torrents start by themselves.': 'टोरेंट अपने आप शुरू होते हैं।',
  'Start torrents by themselves': 'टोरेंट अपने आप शुरू करें',
  'Not checked yet': 'अभी तक नहीं जांचा',
  'Checked just now': 'अभी-अभी जांचा',
  'Checked {n} min ago': '{n} मिनट पहले जांचा',
  'Checked {n} h ago': '{n} घंटे पहले जांचा',
  'Checked {n} days ago': '{n} दिन पहले जांचा',
  '{n} downloaded': 'डाउनलोड: {n}',
  'only with “{words}”': 'सिर्फ़ “{words}” वाले',
  'skipping “{words}”': '“{words}” वाले छोड़कर',
  '{checked}, {every}. {details}.': '{checked}, {every}। {details}।',
  'Check {name} now': '{name} अभी जांचें',
  Filters: 'फ़िल्टर',
  'Stop following {name}': '{name} को फ़ॉलो करना बंद करें',
  'Last check: {problem}': 'पिछली जांच: {problem}',
  'Latest in {name}': '{name} में नया',
  'Only titles with': 'सिर्फ़ इन शब्दों वाले टाइटल',
  'e.g. 1080p': 'जैसे 1080p',
  'Skip titles with': 'इन शब्दों वाले टाइटल छोड़ें',
  'e.g. trailer': 'जैसे trailer',
  Check: 'जांच',
  'Following. The latest file is downloading; new ones follow by themselves.':
    'फ़ॉलो कर रहे हैं। सबसे नई फ़ाइल डाउनलोड हो रही है; नई फ़ाइलें अपने आप आएंगी।',
  'Following. New files download by themselves.':
    'फ़ॉलो कर रहे हैं। नई फ़ाइलें अपने आप डाउनलोड होंगी।',
  "Follow a podcast, a project's releases or any RSS or Atom feed. New files in it download by themselves, over every network. Torrents wait for you to open them, unless you let them start by themselves.":
    'कोई पॉडकास्ट, किसी प्रोजेक्ट की रिलीज़ या कोई भी RSS या Atom फ़ीड फ़ॉलो करें। इसमें आने वाली नई फ़ाइलें हर नेटवर्क से अपने आप डाउनलोड होंगी। टोरेंट तब तक रुके रहेंगे जब तक आप उन्हें न खोलें, या उन्हें अपने आप शुरू होने न दें।',
  'Feed address': 'फ़ीड का पता',
  'Download the latest one now': 'सबसे नई फ़ाइल अभी डाउनलोड करें',
  'Reading the feed…': 'फ़ीड पढ़ रहे हैं…',
  Follow: 'फ़ॉलो करें',
  'Feeds you follow': 'आप जिन फ़ीड को फ़ॉलो करते हैं',
  'No feeds yet. Paste one above to start.':
    'अभी कोई फ़ीड नहीं है। शुरू करने के लिए ऊपर कोई फ़ीड पेस्ट करें।',

  // ==== Send, Nearby and the welcome ====

  // Send page: Fuse Send links (SendView.tsx)
  'Preparing… {pct}%': 'तैयार हो रहा है… {pct}%',
  'Waiting for the receiver': 'पाने वाले का इंतज़ार है',
  '1 receiver connected': '1 पाने वाला जुड़ा है',
  '{n} receivers connected': '{n} पाने वाले जुड़े हैं',
  '{who} · {size} sent': '{who} · {size} भेजा गया',
  'Sent in full. Sharing stopped': 'पूरा भेज दिया। शेयर करना बंद हुआ',
  'File changed': 'फ़ाइल बदल गई',
  "Couldn't share": 'शेयर नहीं हो सका',
  'Link for {name}': '{name} का लिंक',
  'Copy link': 'लिंक कॉपी करें',
  'Stop sending {name}': '{name} भेजना बंद करें',
  Stop: 'बंद करें',
  'Preparing {name}': '{name} तैयार हो रहा है',
  'Stop sharing after one full copy is sent': 'एक पूरी कॉपी भेजने के बाद शेयर करना बंद करें',
  'Looking for the sender…': 'भेजने वाले को ढूंढ रहे हैं…',
  'Receiving…': 'मिल रहा है…',
  'Checking it arrived whole…': 'जांच रहे हैं कि पूरा आया या नहीं…',
  'Arrived and checked': 'मिल गया और जांच लिया',
  "Didn't arrive": 'नहीं पहुंचा',
  'Remove {name} from the list': '{name} को सूची से हटाएं',
  'Receiving {name}': '{name} मिल रहा है',
  'Paste the link someone sent you.': 'किसी का भेजा हुआ लिंक पेस्ट करें।',
  'Link someone sent you': 'किसी का भेजा हुआ लिंक',
  'Opening…': 'खुल रहा है…',
  Receive: 'प्राप्त करें',
  'Saved to your download folder and checked against what the sender shared.':
    'आपके डाउनलोड फ़ोल्डर में सेव होता है और भेजने वाले की शेयर की गई फ़ाइल से जांचा जाता है।',
  Nearby: 'आस-पास',
  'How to send': 'कैसे भेजें',
  'Send to computers and phones on this network. Nothing goes through the internet.':
    'इस नेटवर्क के कंप्यूटर और फ़ोन को भेजें। कुछ भी इंटरनेट से होकर नहीं जाता।',
  'Send a file of any size straight from this computer. No upload, no account, no size limit: the receiver gets it directly from you, encrypted, and only the link can open it.':
    'किसी भी साइज़ की फ़ाइल सीधे इस कंप्यूटर से भेजें। न अपलोड, न अकाउंट, न साइज़ की सीमा: पाने वाले को यह एन्क्रिप्ट होकर सीधे आपसे मिलती है, और इसे सिर्फ़ लिंक से ही खोला जा सकता है।',
  'Send a file': 'फ़ाइल भेजें',
  'Choose a file to send': 'भेजने के लिए फ़ाइल चुनें',
  'You get a link to give to the person receiving it.':
    'आपको एक लिंक मिलेगा, जिसे पाने वाले को दें।',
  "Keep Fuselane open until it arrives. On the same network it connects straight away; across the internet your router needs UPnP on. The key is only in the link, so share it the way you'd share a password.":
    'फ़ाइल पहुंचने तक Fuselane खुला रखें। एक ही नेटवर्क पर यह तुरंत जुड़ जाता है; इंटरनेट के पार आपके राउटर में UPnP चालू होना चाहिए। चाबी सिर्फ़ लिंक में है, इसलिए इसे पासवर्ड की तरह ही शेयर करें।',
  "Files you're sending": 'आप जो फ़ाइलें भेज रहे हैं',
  "Files you're receiving": 'आपको मिल रही फ़ाइलें',

  // Nearby: devices on this network (NearbyPanel.tsx)
  Phone: 'फ़ोन',
  Computer: 'कंप्यूटर',
  '{what}, via LocalSend': '{what}, LocalSend से',
  'No one here yet. Open Fuselane or LocalSend on the other device, on this Wi-Fi.':
    'अभी यहां कोई नहीं है। दूसरे डिवाइस पर, इसी Wi-Fi पर, Fuselane या LocalSend खोलें।',
  'Send files to {name}': '{name} को फ़ाइलें भेजें',
  'Click to choose files, or drop files here':
    'फ़ाइलें चुनने के लिए क्लिक करें, या फ़ाइलें यहां छोड़ें',
  Trusted: 'भरोसेमंद',
  'Waiting for them…': 'उनका इंतज़ार है…',
  'Who can see this computer': 'यह कंप्यूटर कौन देख सकता है',
  'Everyone here, for {time} more.': 'अगले {time} तक यहां सभी देख सकते हैं।',
  'Only devices you trust.': 'सिर्फ़ आपके भरोसेमंद डिवाइस।',
  'Trusted only': 'सिर्फ़ भरोसेमंद',
  'Everyone, 10 min': 'सभी, 10 मिनट',
  'Waiting for them to accept': 'उनके स्वीकार करने का इंतज़ार है',
  Sending: 'भेज रहे हैं',
  Receiving: 'मिल रहा है',
  'They declined': 'उन्होंने मना कर दिया',
  'To {device}.': '{device} को।',
  'From {device}.': '{device} से।',
  'Cancel {name}': '{name} रद्द करें',
  'Copy text from {name}': '{name} से आया टेक्स्ट कॉपी करें',
  'Clear {name}': '{name} हटाएं',
  'Send text': 'टेक्स्ट भेजें',
  'Text to send': 'भेजने के लिए टेक्स्ट',
  'Type or paste, or leave empty to send what you copied':
    'लिखें या पेस्ट करें, या कॉपी किया हुआ भेजने के लिए खाली छोड़ें',
  'Send to': 'किसे भेजें',
  'Send what I copied': 'कॉपी किया हुआ भेजें',
  'Up to date': 'सब अपडेट है',
  'Up to date, {n} file': 'सब अपडेट है, {n} फ़ाइल',
  'Up to date, {n} files': 'सब अपडेट है, {n} फ़ाइलें',
  'Sending changes': 'बदलाव भेज रहे हैं',
  'Sending changes, {n} to go': 'बदलाव भेज रहे हैं, {n} बाकी',
  Problem: 'समस्या',
  'Folders kept in sync': 'सिंक में रखे फ़ोल्डर',
  'Stop syncing {name}': '{name} का सिंक बंद करें',
  'Keep in sync with': 'इसके साथ सिंक रखें',
  'Add a folder': 'फ़ोल्डर जोड़ें',
  "New and changed files go to that computer's downloads folder whenever both are on this network. Nothing is deleted there.":
    'जब भी दोनों इस नेटवर्क पर हों, नई और बदली हुई फ़ाइलें उस कंप्यूटर के डाउनलोड फ़ोल्डर में चली जाती हैं। वहां कुछ भी मिटाया नहीं जाता।',
  'A phone without Fuselane?': 'फ़ोन में Fuselane नहीं है?',
  'It can send and receive in its browser, on this Wi-Fi.':
    'वह इसी Wi-Fi पर अपने ब्राउज़र में भेज और पा सकता है।',
  'Show a code to scan': 'स्कैन के लिए कोड दिखाएं',
  "Code to scan with the phone's camera": 'फ़ोन के कैमरे से स्कैन करने का कोड',
  "Scan with the phone's camera": 'फ़ोन के कैमरे से स्कैन करें',
  'Or type {url}': 'या {url} लिखें',
  'Offered to the phone': 'फ़ोन के लिए रखी गई फ़ाइलें',
  'Stop offering {name}': '{name} देना बंद करें',
  "On the phone's page: {text}": 'फ़ोन के पेज पर: {text}',
  "Take the text away from the phone's page": 'फ़ोन के पेज से टेक्स्ट हटाएं',
  'Text for the phone': 'फ़ोन के लिए टेक्स्ट',
  'Text or a link for the phone, or leave empty for what you copied':
    'फ़ोन के लिए टेक्स्ट या लिंक, या कॉपी किए हुए के लिए खाली छोड़ें',
  'Offer text': 'टेक्स्ट दें',
  'Offer what I copied': 'कॉपी किया हुआ दें',
  "Text typed on the phone lands on this computer's clipboard.":
    'फ़ोन पर लिखा टेक्स्ट इस कंप्यूटर के क्लिपबोर्ड पर आ जाता है।',
  'Offer files to the phone': 'फ़ोन को फ़ाइलें दें',
  'Looking for devices': 'डिवाइस ढूंढ रहे हैं',
  'On this network {n}': 'इस नेटवर्क पर {n}',
  'Click a device or drop files on it. Files go straight there, encrypted.':
    'किसी डिवाइस पर क्लिक करें या उस पर फ़ाइलें छोड़ें। फ़ाइलें एन्क्रिप्ट होकर सीधे वहां जाती हैं।',
  Activity: 'गतिविधि',
  'Nothing sent or received yet.': 'अभी तक कुछ भेजा या पाया नहीं गया।',
  'Trusted devices {n}': 'भरोसेमंद डिवाइस {n}',
  'Since {date}. Sends without asking.': '{date} से। बिना पूछे भेजता है।',
  'Forget {name}': '{name} को भूल जाएं',
  Forget: 'भूल जाएं',
  '{name} wants to send you text': '{name} आपको टेक्स्ट भेजना चाहता है',
  '{name} wants to send you a file': '{name} आपको एक फ़ाइल भेजना चाहता है',
  '{name} wants to send you {n} files': '{name} आपको {n} फ़ाइलें भेजना चाहता है',
  '{n} file': '{n} फ़ाइल',
  '{n} files': '{n} फ़ाइलें',
  '{size}, saves to your downloads folder': '{size}, आपके डाउनलोड फ़ोल्डर में सेव होगा',
  'Trust {name}. Its files arrive without asking next time, and its text goes straight to your clipboard.':
    '{name} पर भरोसा करें। अगली बार इसकी फ़ाइलें बिना पूछे आएंगी, और इसका टेक्स्ट सीधे आपके क्लिपबोर्ड पर जाएगा।',
  Decline: 'मना करें',
  Accept: 'स्वीकार करें',

  // Welcome (Welcome.tsx)
  'Phone or second network not showing up?': 'फ़ोन या दूसरा नेटवर्क नहीं दिख रहा?',
  '{label} turn on Personal Hotspot, then connect it with a USB cable. It shows up as iPhone USB.':
    '{label} पर्सनल हॉटस्पॉट चालू करें, फिर इसे USB केबल से जोड़ें। यह iPhone USB के नाम से दिखेगा।',
  "{label} macOS can't use Android USB tethering by itself. It needs a third-party driver such as TetherKit. Joining the phone's Wi‑Fi hotspot instead replaces your Wi‑Fi, so it doesn't add a network.":
    '{label} macOS अपने आप Android की USB टेदरिंग इस्तेमाल नहीं कर सकता। इसके लिए TetherKit जैसा कोई थर्ड-पार्टी ड्राइवर चाहिए। इसकी जगह फ़ोन के Wi‑Fi हॉटस्पॉट से जुड़ने पर आपका Wi‑Fi बदल जाता है, इसलिए नया नेटवर्क नहीं जुड़ता।',
  'Phone:': 'फ़ोन:',
  '{label} connect it with a USB cable and turn on USB tethering (Android: Settings, Network, Hotspot and tethering; iPhone: Personal Hotspot).':
    '{label} इसे USB केबल से जोड़ें और USB टेदरिंग चालू करें (Android: सेटिंग्स, नेटवर्क, हॉटस्पॉट और टेदरिंग; iPhone: पर्सनल हॉटस्पॉट)।',
  'Wi‑Fi turns off when Ethernet is plugged in?': 'Ethernet लगाने पर Wi‑Fi बंद हो जाता है?',
  '{question} Windows does that to save connections. In the Group Policy editor, open Computer Configuration, Administrative Templates, Network, Windows Connection Manager, and set "Minimize the number of simultaneous connections to the Internet or a Windows Domain" to Disabled. On Windows Home, set the registry value {value} to 0 under {key}.':
    '{question} Windows कनेक्शन बचाने के लिए ऐसा करता है। Group Policy एडिटर में Computer Configuration, Administrative Templates, Network, Windows Connection Manager खोलें, और "Minimize the number of simultaneous connections to the Internet or a Windows Domain" को Disabled करें। Windows Home पर {key} में रजिस्ट्री वैल्यू {value} को 0 करें।',
  'Wi‑Fi and Ethernet together:': 'Wi‑Fi और Ethernet एक साथ:',
  '{label} connect both. A network joins as soon as it has an address; no restart needed.':
    '{label} दोनों जोड़ें। पता मिलते ही नेटवर्क जुड़ जाता है; रीस्टार्ट की ज़रूरत नहीं।',
  "The check couldn't reach the test server. You can run it later from Networks.":
    'जांच टेस्ट सर्वर तक नहीं पहुंच सकी। आप इसे बाद में नेटवर्क पेज से चला सकते हैं।',
  'Together: {together}, about the same as {name} alone. One connection is likely the limit, not the networks.':
    'एक साथ: {together}, लगभग अकेले {name} जितना। रुकावट शायद एक कनेक्शन की सीमा है, नेटवर्क नहीं।',
  'Together: {together}, {times} times {name} alone ({top}).':
    'एक साथ: {together}, अकेले {name} ({top}) का {times} गुना।',
  'Step {n} of {total}': 'चरण {n} / {total}',
  'Welcome to Fuselane': 'Fuselane में आपका स्वागत है',
  'Fuselane splits each download across every network this computer has, then joins the parts into one file.':
    'Fuselane हर डाउनलोड को इस कंप्यूटर के सभी नेटवर्क में बांटता है, फिर हिस्सों को जोड़कर एक फ़ाइल बनाता है।',
  'Networks found': 'मिले नेटवर्क',
  'No internet': 'इंटरनेट नहीं',
  'Needs a sign-in page': 'साइन-इन पेज चाहिए',
  Ready: 'तैयार',
  'Looking for networks…': 'नेटवर्क ढूंढ रहे हैं…',
  'One network so far. To go faster, plug in your phone with a USB cable and turn on USB tethering (Personal Hotspot on iPhone), or use Wi‑Fi and Ethernet together. New networks join by themselves.':
    'अभी एक ही नेटवर्क है। तेज़ी के लिए अपना फ़ोन USB केबल से जोड़ें और USB टेदरिंग चालू करें (iPhone पर पर्सनल हॉटस्पॉट), या Wi‑Fi और Ethernet एक साथ इस्तेमाल करें। नए नेटवर्क अपने आप जुड़ जाते हैं।',
  'How fast are they together?': 'एक साथ ये कितने तेज़ हैं?',
  'A quick check measures each network, then all of them at once. It takes under a minute and runs each network flat out for a few seconds, so skip it on a tight data plan.':
    'एक छोटी जांच हर नेटवर्क को, फिर सभी को एक साथ मापती है। इसमें एक मिनट से कम लगता है और हर नेटवर्क कुछ सेकंड पूरी स्पीड पर चलता है, इसलिए कम डेटा वाले प्लान पर इसे छोड़ दें।',
  'Check results': 'जांच के नतीजे',
  'No answer': 'कोई जवाब नहीं',
  'Run the check': 'जांच चलाएं',
  'Ready when you are': 'सब तैयार है',
  '{paste} in the window, or press {keys}.': 'विंडो में {paste}, या {keys} दबाएं।',
  'Paste a link anywhere': 'कहीं भी लिंक पेस्ट करें',
  '{extension} hands big downloads to Fuselane by itself. Get it from fuselane.app.':
    '{extension} बड़े डाउनलोड अपने आप Fuselane को सौंप देता है। इसे fuselane.app से पाएं।',
  'The browser extension': 'ब्राउज़र एक्सटेंशन',
  '{label} {folder}. Change it for any download.':
    '{label} {folder}। किसी भी डाउनलोड के लिए इसे बदल सकते हैं।',
  'Files go to': 'फ़ाइलें यहां सेव होती हैं:',
  'your Downloads folder': 'आपका डाउनलोड फ़ोल्डर',
  'Start downloading': 'डाउनलोड शुरू करें',

  // ==== Networks, and the automation settings ====

  // NetworksView: the sidebar total and the network list
  'All downloads together': 'सभी डाउनलोड मिलाकर',
  '{name} wants you to sign in before it reaches the internet, so downloads leave it out for now. Fuselane checks again every minute.':
    'इंटरनेट चलाने से पहले {name} पर साइन इन करना होगा, इसलिए अभी डाउनलोड इसका इस्तेमाल नहीं करते। Fuselane हर मिनट फिर से जांचता है।',
  'Open sign-in page': 'साइन-इन पेज खोलें',
  'No networks found. Join Wi-Fi, plug in Ethernet, or tether a phone over USB.':
    'कोई नेटवर्क नहीं मिला। Wi-Fi से जुड़ें, Ethernet लगाएं या USB से फ़ोन टेदर करें।',
  'Sign in needed': 'साइन इन ज़रूरी',
  'Rename or recolour {name}': '{name} का नाम या रंग बदलें',
  'Rename or recolour': 'नाम या रंग बदलें',

  // NetworksView: renaming and recolouring a network
  'Use up to 40 characters.': 'ज़्यादा से ज़्यादा 40 अक्षर लिखें।',
  Colour: 'रंग',
  tide: 'फ़िरोज़ी',
  volt: 'हरा',
  iris: 'बैंगनी',
  rose: 'गुलाबी',
  mint: 'समुद्री हरा',
  sky: 'आसमानी',
  lilac: 'हल्का बैंगनी',
  steel: 'स्लेटी',
  Reset: 'रीसेट करें',

  // NetworksView: when each network helps, and its hours
  'Only from': 'सिर्फ़ इस समय:',
  '{start} to {end}': '{start} से {end} तक',
  'Before midnight': 'आधी रात से पहले',
  '{name}: only before midnight, 22:00 to 00:00':
    '{name}: सिर्फ़ आधी रात से पहले, 22:00 से 00:00 तक',
  "22:00 to midnight. Daily data packs expire at midnight, so the phone helps with what's left of today's data. If it runs out, Fuselane notices the slowdown and stops using it.":
    '22:00 से आधी रात तक। रोज़ के डेटा पैक आधी रात को खत्म हो जाते हैं, इसलिए फ़ोन आज के बचे डेटा से मदद करता है। डेटा खत्म हो जाए, तो Fuselane धीमी स्पीड पहचानकर उसका इस्तेमाल बंद कर देता है।',
  '{name} from': '{name}: कब से',
  '{name} until': '{name}: कब तक',
  Always: 'हमेशा',
  'Long downloads': 'लंबे डाउनलोड',
  Never: 'कभी नहीं',
  'A long download takes more than': 'लंबा डाउनलोड यानी उनके बिना',
  'minutes without them.': 'मिनट से ज़्यादा।',
  "Downloads start without them; once a download's speed shows it's long, they join in and it carries on where it was.":
    'डाउनलोड उनके बिना शुरू होते हैं; स्पीड से पता चलते ही कि डाउनलोड लंबा है, वे भी जुड़ जाते हैं और डाउनलोड वहीं से आगे चलता है।',

  // NetworksView: speed limits and monthly allowances
  '{name} speed limit': '{name} की स्पीड लिमिट',
  '{used} of {allowance}, resets {date}': '{allowance} में से {used}, {date} को रीसेट',
  '{used} used, resets {date}': '{used} इस्तेमाल हुआ, {date} को रीसेट',
  "Allowance reached: Fuselane won't use this network until {date}.":
    'लिमिट पूरी हो गई: Fuselane {date} तक इस नेटवर्क का इस्तेमाल नहीं करेगा।',
  '{name} monthly allowance': '{name} की मासिक डेटा लिमिट',
  '{name} allowance unit': '{name} की लिमिट की इकाई',
  'Resets on day': 'रीसेट की तारीख',
  '{name} reset day': '{name} के रीसेट की तारीख',

  // NetworksView: the page, its tabs and networks not used
  Setup: 'सेटअप',
  Usage: 'इस्तेमाल',
  'Networks view': 'नेटवर्क टैब',
  Refresh: 'रीफ़्रेश करें',
  'Not used ({n})': 'इस्तेमाल में नहीं ({n})',
  'Tunnels are skipped so traffic stays where you expect.':
    'टनल छोड़ दिए जाते हैं, ताकि ट्रैफ़िक वहीं रहे जहां आप चाहते हैं।',
  'Not connected to the internet.': 'इंटरनेट से जुड़ा नहीं है।',

  // NetCheck
  'Gets very slow while busy: calls and games stutter when something downloads.':
    'व्यस्त होने पर बहुत धीमा: कुछ डाउनलोड होते समय कॉल और गेम अटकते हैं।',
  'Slows down while busy: a big download makes calls lag.':
    'व्यस्त होने पर धीमा: बड़े डाउनलोड से कॉल में देरी होती है।',
  'Drops some connections.': 'कुछ कनेक्शन टूट जाते हैं।',
  'Uneven: fine for downloads, choppy for calls.':
    'अस्थिर: डाउनलोड के लिए ठीक, कॉल रुक-रुक कर चलती है।',
  'Healthy.': 'ठीक है।',
  'Download (Mbps)': 'डाउनलोड (Mbps)',
  'Every network together': 'सभी नेटवर्क एक साथ',
  '{name}: sign-in page from {from}, still going on': '{name}: {from} से साइन-इन पेज, अब भी जारी',
  '{name}: sign-in page from {from}, {m} min': '{name}: {from} से साइन-इन पेज, {m} मिनट',
  '{name}: offline from {from}, still going on': '{name}: {from} से ऑफ़लाइन, अब भी जारी',
  '{name}: offline from {from}, {m} min': '{name}: {from} से ऑफ़लाइन, {m} मिनट',
  'No outages in the last 7 days.': 'पिछले 7 दिनों में इंटरनेट एक बार भी नहीं गया।',
  '{name}: no internet once this week, {m} min in total.':
    '{name}: इस हफ़्ते एक बार इंटरनेट गया, कुल {m} मिनट।',
  '{name}: no internet {n} times this week, {m} min in total.':
    '{name}: इस हफ़्ते {n} बार इंटरनेट गया, कुल {m} मिनट।',
  Speedtest: 'स्पीडटेस्ट',
  'Save the speed limit for {name}': '{name} की स्पीड सीमा सेव करें',
  'Your networks, joined': 'आपके नेटवर्क, एक साथ',
  Together: 'एक साथ',
  '1 network': '1 नेटवर्क',
  'Helps with': 'कब मदद करे',
  'Monthly data': 'महीने का डेटा',
  Proxy: 'प्रॉक्सी',
  'Helps with: a phone on a data plan can wait for the downloads where it makes a real difference. Speed limit: cap a network and the others carry the rest. Monthly data: when a network reaches its allowance, Fuselane stops using it until the reset day.':
    'कब मदद करे: डेटा प्लान वाला फ़ोन उन डाउनलोड का इंतज़ार कर सकता है जहां उससे सच में फ़र्क पड़ता है। स्पीड सीमा: किसी नेटवर्क की सीमा तय करें, बाकी नेटवर्क बाकी काम करते हैं। महीने का डेटा: जब कोई नेटवर्क अपनी सीमा तक पहुंच जाता है, तो Fuselane रीसेट के दिन तक उसका इस्तेमाल नहीं करता।',
  "Proxy: for a network that only reaches the internet through one; https stays encrypted end to end. Torrents don't use these proxies, and proxy settings from your system aren't used, only the ones set here.":
    'प्रॉक्सी: ऐसे नेटवर्क के लिए जो सिर्फ़ प्रॉक्सी से इंटरनेट तक पहुंचता है; https शुरू से आखिर तक एन्क्रिप्टेड रहता है। टोरेंट इन प्रॉक्सी का इस्तेमाल नहीं करते, और आपके सिस्टम की प्रॉक्सी सेटिंग नहीं, सिर्फ़ यहां सेट की गई इस्तेमाल होती हैं।',
  'Loading…': 'लोड हो रहा है…',
  'Nothing counted yet': 'अभी कुछ गिना नहीं गया',
  'Everything Fuselane downloads is counted here, per network and day by day, so you can see how much the phone carried. Start a download and watch it fill in.':
    'Fuselane जो भी डाउनलोड करता है, वह यहां हर नेटवर्क और हर दिन के हिसाब से गिना जाता है, ताकि आप देख सकें फ़ोन ने कितना डेटा चलाया। कोई डाउनलोड शुरू करें और इसे भरते देखें।',
  'This month': 'इस महीने',
  '{size} today': 'आज {size}',
  'Share of each network this month': 'इस महीने हर नेटवर्क का हिस्सा',
  'Right now': 'अभी',
  Idle: 'खाली',
  'Last {n} days': 'पिछले {n} दिन',
  'About {size} a day': 'रोज़ लगभग {size}',
  Today: 'आज',
  '{used} of {limit}, resets {date}': '{limit} में से {used}, {date} को फिर से शुरू',
  'Finding the speed server': 'स्पीड सर्वर ढूंढा जा रहा है',
  Ping: 'पिंग',
  Upload: 'अपलोड',
  Jitter: 'जिटर',
  'While busy': 'व्यस्त होने पर',
  DNS: 'DNS',
  Loss: 'नुकसान',
  Steps: 'चरण',
  When: 'कब',
  Outages: 'इंटरनेट कब गया',
  Again: 'फिर से',
  '{n} ms': '{n} ms',
  'server in {city}': 'सर्वर {city} में',
  'How well it copes when busy, A+ to F': 'व्यस्त होने पर यह कितना अच्छा चलता है, A+ से F तक',
  'Used {size}.': '{size} खर्च हुआ।',
  'What Fuselane can pull in at once, for one download.':
    'एक डाउनलोड के लिए Fuselane एक साथ कितना ला सकता है।',
  '{x}× your fastest network': 'आपके सबसे तेज़ नेटवर्क से {x} गुना',
  'Earlier tests ({n})': 'पिछले टेस्ट ({n})',
  'Upload (Mbps)': 'अपलोड (Mbps)',
  'Ping (ms)': 'पिंग (ms)',
  'Testing {network}': '{network} जांचा जा रहा है',
  'Last test {when}.': 'आखिरी टेस्ट: {when}।',
  'Measures each network on its own (ping, download, upload, and how it copes when busy), then every network together.':
    'हर नेटवर्क को अलग से मापता है (पिंग, डाउनलोड, अपलोड, और व्यस्त होने पर वह कैसा चलता है), फिर सभी नेटवर्क एक साथ।',
  'About {s} seconds. Each network runs flat out for a few seconds, the way public speed tests do, so it uses data on metered connections.':
    'लगभग {s} सेकंड। सार्वजनिक स्पीड टेस्ट की तरह हर नेटवर्क कुछ सेकंड पूरी स्पीड पर चलता है, इसलिए सीमित डेटा वाले कनेक्शन पर डेटा खर्च होता है।',
  'A dated page with every check and outage, to send your internet provider (print it to PDF)':
    'हर जांच और इंटरनेट जाने के समय का तारीख वाला पेज, अपने इंटरनेट प्रोवाइडर को भेजने के लिए (PDF में प्रिंट करें)',
  'Report for your provider': 'प्रोवाइडर के लिए रिपोर्ट',

  // DataUsed
  'Data used': 'इस्तेमाल हुआ डेटा',
  'Data used per day for the last {n} days': 'पिछले {n} दिनों में हर दिन इस्तेमाल हुआ डेटा',

  // AutomationSettings: downloads at once and the schedule
  Mon: 'सोम',
  Tue: 'मंगल',
  Wed: 'बुध',
  Thu: 'गुरु',
  Fri: 'शुक्र',
  Sat: 'शनि',
  Sun: 'रवि',
  Monday: 'सोमवार',
  Tuesday: 'मंगलवार',
  Wednesday: 'बुधवार',
  Thursday: 'गुरुवार',
  Friday: 'शुक्रवार',
  Saturday: 'शनिवार',
  Sunday: 'रविवार',
  'One at a time.': 'एक बार में एक।',
  'Up to {n} at once.': 'एक साथ {n} तक।',
  "Couldn't save that. Try again.": 'सेव नहीं हो सका। फिर से कोशिश करें।',
  'Downloads at once': 'एक साथ कितने डाउनलोड',
  'Each one already uses every network. More start as others finish.':
    'हर डाउनलोड पहले से सभी नेटवर्क इस्तेमाल करता है। बाकी डाउनलोड दूसरों के पूरे होने पर शुरू होते हैं।',
  'One fewer': 'एक कम',
  'One more': 'एक और',
  'Not saved. Check the times and days.': 'सेव नहीं हुआ। समय और दिन जांचें।',
  'Download schedule': 'डाउनलोड शेड्यूल',
  'Enter times like 01:00.': '01:00 जैसा समय लिखें।',
  Schedule: 'शेड्यूल',
  'Only download between two times, for example at night when data is cheap. Downloads outside it wait, and running ones pause and carry on next time.':
    'सिर्फ़ दो समय के बीच डाउनलोड करें, जैसे रात में जब डेटा सस्ता हो। बाकी समय डाउनलोड इंतज़ार करते हैं, और चल रहे डाउनलोड रुककर अगली बार आगे चलते हैं।',
  'Download only on a schedule': 'सिर्फ़ शेड्यूल पर डाउनलोड करें',
  From: 'कब से',
  Until: 'कब तक',
  Days: 'दिन',
  'Runs overnight: from {start} until {stop} the next morning.':
    'रात भर चलेगा: {start} से अगली सुबह {stop} तक।',

  // AutomationSettings: when everything finishes, and this computer
  Nothing: 'कुछ नहीं',
  Sleep: 'स्लीप',
  'Shut down': 'शट डाउन',
  Quit: 'ऐप बंद करें',
  'When everything finishes': 'सब पूरा होने पर',
  'Fuselane just waits.': 'Fuselane बस इंतज़ार करता है।',
  'You get 60 seconds and a notification to cancel first.':
    'रद्द करने के लिए आपको 60 सेकंड और एक सूचना मिलती है।',
  'Keep the computer awake': 'कंप्यूटर को जगाए रखें',
  "While something downloads, your computer won't go to sleep. The screen can still turn off.":
    'कुछ डाउनलोड होते समय कंप्यूटर स्लीप में नहीं जाएगा। स्क्रीन फिर भी बंद हो सकती है।',
  'Downloads carry on whatever the battery.': 'बैटरी कितनी भी हो, डाउनलोड चलते रहते हैं।',
  'Under 20% and unplugged, a phone tethered by cable is left out (it charges from this computer).':
    'बैटरी 20% से कम हो और चार्जर न लगा हो, तो केबल से जुड़ा फ़ोन छोड़ दिया जाता है (वह इसी कंप्यूटर से चार्ज होता है)।',
  'Under 20% and unplugged, downloads pause and carry on when you plug in.':
    'बैटरी 20% से कम हो और चार्जर न लगा हो, तो डाउनलोड रुक जाते हैं और चार्जर लगाने पर आगे चलते हैं।',
  'On low battery': 'बैटरी कम होने पर',
  'Keep going': 'चलते रहें',
  'Leave out the phone': 'फ़ोन छोड़ दें',
  'Start at login': 'लॉगिन पर शुरू करें',
  'Opens quietly in the tray when you sign in, so scheduled downloads run.':
    'साइन इन करते ही चुपचाप ट्रे में खुल जाता है, ताकि शेड्यूल वाले डाउनलोड चल सकें।',
  "Couldn't change it. Try again.": 'बदल नहीं सका। फिर से कोशिश करें।',
  'Keep running when the window closes': 'विंडो बंद होने पर भी चलता रहे',
  'Closing the window leaves Fuselane in the tray and downloads carry on. Quit from the tray menu.':
    'विंडो बंद करने पर Fuselane ट्रे में रहता है और डाउनलोड चलते रहते हैं। पूरी तरह बंद करने के लिए ट्रे मेन्यू इस्तेमाल करें।',
  'Catch copied download links': 'कॉपी किए गए डाउनलोड लिंक पकड़ें',
  'When you copy a link to a file (a .zip, .iso, video and so on) or a magnet, Fuselane offers to download it. The clipboard is only read on this computer while this is on.':
    'जब आप किसी फ़ाइल (.zip, .iso, वीडियो वगैरह) या मैग्नेट का लिंक कॉपी करते हैं, तो Fuselane उसे डाउनलोड करने के लिए पूछता है। यह चालू रहने पर क्लिपबोर्ड सिर्फ़ इसी कंप्यूटर पर पढ़ा जाता है।',

  // AutomationSettings: files
  'New download asks when a file with the same name is already in the folder.':
    'फ़ोल्डर में इसी नाम की फ़ाइल पहले से हो, तो नया डाउनलोड पूछेगा।',
  'The new file is saved as "name (1)" next to the old one.':
    'नई फ़ाइल पुरानी के पास "नाम (1)" के रूप में सेव होती है।',
  'The old file goes to the Recycle Bin once the new one is complete.':
    'नई फ़ाइल पूरी होते ही पुरानी फ़ाइल रीसायकल बिन में चली जाती है।',
  'The old file goes to the Trash once the new one is complete.':
    'नई फ़ाइल पूरी होते ही पुरानी फ़ाइल ट्रैश में चली जाती है।',
  'If the name is taken': 'अगर नाम पहले से हो',
  Ask: 'पूछें',
  'Finished files stay where they are.': 'पूरी हुई फ़ाइलें जहां हैं, वहीं रहती हैं।',
  'Each finished file opens with its usual app.': 'हर पूरी हुई फ़ाइल अपने सामान्य ऐप में खुलती है।',
  'Zip and tar archives unpack into a folder next to them. The archive is kept.':
    'Zip और tar आर्काइव पास के एक फ़ोल्डर में खुल जाते हैं। आर्काइव भी रखा जाता है।',
  'When a download finishes': 'डाउनलोड पूरा होने पर',
  'Open it': 'खोलें',
  'Unpack it': 'अनपैक करें',
  'Sort into folders by type': 'प्रकार के हिसाब से फ़ोल्डरों में रखें',
  'Finished files go into Video, Music, Documents, Compressed and similar folders inside {folder}. A folder you pick yourself is left alone.':
    'पूरी हुई फ़ाइलें {folder} के अंदर Video, Music, Documents, Compressed जैसे फ़ोल्डरों में जाती हैं। आपका खुद चुना फ़ोल्डर जैसा है वैसा रहता है।',
  'your downloads folder': 'आपके डाउनलोड फ़ोल्डर',

  // RemoteSetting
  'Remote control for aria2 apps': 'aria2 ऐप के लिए रिमोट कंट्रोल',
  'AriaNg, the Aria2 browser extensions and phone remote apps can add downloads here and watch them. What they add uses every network, like anything else.':
    'AriaNg, Aria2 ब्राउज़र एक्सटेंशन और फ़ोन के रिमोट ऐप यहां डाउनलोड जोड़ सकते हैं और उन पर नज़र रख सकते हैं। वे जो जोड़ते हैं, वह भी बाकी सब की तरह हर नेटवर्क इस्तेमाल करता है।',
  'Remote control is off.': 'रिमोट कंट्रोल बंद है।',
  Address: 'पता',
  'Copy {address}': '{address} कॉपी करें',
  'Address copied.': 'पता कॉपी हो गया।',
  'Select the address and copy it.': 'पता चुनकर कॉपी करें।',
  Secret: 'सीक्रेट',
  'Hide the secret': 'सीक्रेट छिपाएं',
  'Show the secret': 'सीक्रेट दिखाएं',
  'Copy the secret': 'सीक्रेट कॉपी करें',
  'Secret copied.': 'सीक्रेट कॉपी हो गया।',
  'Select the secret and copy it.': 'सीक्रेट चुनकर कॉपी करें।',
  'Apps using the old secret stop working until you give them the new one':
    'पुराना सीक्रेट इस्तेमाल करने वाले ऐप तब तक काम नहीं करेंगे, जब तक आप उन्हें नया सीक्रेट न दें',
  'New secret made.': 'नया सीक्रेट बन गया।',
  'New secret': 'नया सीक्रेट',
  'In AriaNg: AriaNg Settings, then RPC. Enter the address (WebSocket or HTTP both work) and the secret.':
    'AriaNg में: AriaNg Settings, फिर RPC। पता (WebSocket या HTTP, दोनों चलते हैं) और सीक्रेट डालें।',
  'Allow phones and computers on this network': 'इस नेटवर्क के फ़ोन और कंप्यूटर को अनुमति दें',
  'The secret travels unencrypted on the network, so use this only at home or work.':
    'सीक्रेट नेटवर्क पर बिना एन्क्रिप्शन के जाता है, इसलिए इसे सिर्फ़ घर या दफ़्तर में इस्तेमाल करें।',
  'Code to scan with your phone': 'फ़ोन से स्कैन करने का कोड',
  "Scan with your phone's camera": 'फ़ोन के कैमरे से स्कैन करें',
  'A page opens with your downloads: add links, pause and resume. The code carries the secret, so show it only to your own phone.':
    'आपके डाउनलोड वाला एक पेज खुलेगा: लिंक जोड़ें, रोकें और फिर शुरू करें। कोड में सीक्रेट होता है, इसलिए इसे सिर्फ़ अपने फ़ोन को दिखाएं।',
  'Hide the code': 'कोड छिपाएं',
  'Show a code for your phone': 'फ़ोन के लिए कोड दिखाएं',
  Port: 'पोर्ट',
  '6800 is what aria2 apps expect.': 'aria2 ऐप 6800 पोर्ट इस्तेमाल करते हैं।',

  // WatchSetting
  'Add files from a folder': 'फ़ोल्डर से फ़ाइलें जोड़ें',
  'Put a .torrent file, a Metalink or a .txt list of links in this folder and it starts by itself; the file is then renamed to end in .added. Media tools that save .torrent files to a folder work with it.':
    'इस फ़ोल्डर में .torrent फ़ाइल, Metalink या लिंक की .txt सूची रखें, डाउनलोड अपने आप शुरू हो जाएगा; फिर फ़ाइल के नाम के आखिर में .added जुड़ जाता है। जो मीडिया टूल .torrent फ़ाइलें किसी फ़ोल्डर में सेव करते हैं, वे इसके साथ काम करते हैं।',
  'Change…': 'बदलें…',
  'Files taken from the folder': 'फ़ोल्डर से ली गई फ़ाइलें',
  Added: 'जोड़ा गया',
  'Not added': 'नहीं जोड़ा गया',
  'Watching. Nothing has been put in it yet.': 'नज़र रखी जा रही है। अभी तक इसमें कुछ नहीं रखा गया।',

  // NetworkProxy: a proxy per network
  'Direct, no proxy': 'सीधे, बिना प्रॉक्सी',
  '{proxy}, as {user}': '{proxy}, यूज़र {user}',
  'Check the proxy for {network}': '{network} का प्रॉक्सी जांचें',
  'Edit the proxy for {network}': '{network} का प्रॉक्सी बदलें',
  'Set up a proxy for {network}': '{network} के लिए प्रॉक्सी सेट करें',
  Edit: 'बदलें',
  'Set up': 'सेट करें',
  'Checking the proxy…': 'प्रॉक्सी जांच रहे हैं…',
  'Proxy for {network}': '{network} का प्रॉक्सी',
  "Enter the proxy's name or address.": 'प्रॉक्सी का नाम या पता लिखें।',
  'Use a port from 1 to 65535, like 8080 or 1080.':
    '1 से 65535 तक का पोर्ट लिखें, जैसे 8080 या 1080।',
  Username: 'यूज़रनेम',
  Password: 'पासवर्ड',
  '(if it asks)': '(अगर मांगे)',
  'A password is saved and never shown. Leave the box empty to keep it.':
    'पासवर्ड सेव है और कभी दिखाया नहीं जाता। उसे रखने के लिए बॉक्स खाली छोड़ें।',
  "A password is saved in your system's keychain and never shown. Leave the box empty to keep it.":
    'पासवर्ड आपके सिस्टम के कीचेन में सेव है और कभी दिखाया नहीं जाता। उसे रखने के लिए बॉक्स खाली छोड़ें।',
  "A password is saved in Fuselane's settings (no system keychain available) and never shown. Leave the box empty to keep it.":
    'पासवर्ड Fuselane की सेटिंग्स में सेव है (कोई सिस्टम कीचेन उपलब्ध नहीं) और कभी दिखाया नहीं जाता। उसे रखने के लिए बॉक्स खाली छोड़ें।',
  'Forget the saved password': 'सेव किया पासवर्ड भूल जाएं',
  'Remove proxy': 'प्रॉक्सी हटाएं',
  'Saving…': 'सेव हो रहा है…',

  // NetworkNotes: what happened to a download's networks
  'Network notes': 'नेटवर्क की जानकारी',
  '{network} slowed to {rate} (throttled?), so the other networks carry the rest. Fuselane tries it again every few minutes.':
    '{network} की स्पीड घटकर {rate} रह गई (शायद धीमी की गई?), इसलिए बाकी काम दूसरे नेटवर्क करेंगे। Fuselane हर कुछ मिनट में इसे फिर आज़माता है।',
  '{network} is only managing {rate} (throttled?), so the other networks carry the rest. Fuselane tries it again every few minutes.':
    '{network} सिर्फ़ {rate} दे पा रहा है (शायद धीमा किया गया?), इसलिए बाकी काम दूसरे नेटवर्क करेंगे। Fuselane हर कुछ मिनट में इसे फिर आज़माता है।',
  "{network} is fast again ({rate}), so it's helping again.":
    '{network} फिर से तेज़ है ({rate}), इसलिए यह फिर से मदद कर रहा है।',
  "{network} can't connect.": '{network} कनेक्ट नहीं हो पा रहा।',
}
